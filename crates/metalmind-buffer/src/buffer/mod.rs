use std::fmt;
use std::ops::Range;

use ropey::Rope;

mod anchored;
mod lines;
mod marked;
mod text;
mod undo;

use crate::anchor::{Anchors, LineChange, Replacement};
use crate::history::{AppliedEdit, History};
use crate::line_move::LineMove;
use crate::marked::MarkedLines;
use crate::spot::Spot;
use crate::{
    BufferError, ByteOffset, Edit, EditMap, HistorySeq, LineEdit, Selections, Snapshot, Splice,
};

/// Texto editable. Las líneas se separan solo por `\n`.
#[derive(Debug, Default)]
pub struct Buffer {
    rope: Rope,
    history: History,
    anchors: Anchors,
    marked: MarkedLines,
    version: u64,
    changes: Changes,
}

/// Lo reemplazado, para quien guarda posiciones o líneas fuera del buffer.
#[derive(Debug, Default)]
struct Changes {
    /// Desde el último [`Buffer::take_splices`].
    splices: Vec<Splice>,
    /// Los que ya se sacaron con [`Buffer::take_splices`].
    taken: usize,
    /// Lo mismo por líneas, desde el último [`Buffer::take_line_edits`].
    lines: Vec<LineEdit>,
}

impl Buffer {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Crece con cada edición o paso por el historial; sirve para descartar
    /// cálculos viejos.
    #[must_use]
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Los reemplazos hechos desde la última llamada, en orden, incluidos
    /// los de undo y redo. Quien guarde posiciones fuera del motor (el
    /// cursor de otro panel) las mueve con [`Splice::map_all`] y lo llama
    /// tras cada cambio para que la lista no crezca.
    pub fn take_splices(&mut self) -> Vec<Splice> {
        let splices = std::mem::take(&mut self.changes.splices);
        self.changes.taken += splices.len();
        splices
    }

    /// Los reemplazos desde la última llamada, por líneas y en orden. Es
    /// otra lista que la de [`Buffer::take_splices`]: la lee otro (el
    /// estilo de las líneas) en otro momento.
    pub fn take_line_edits(&mut self) -> Vec<LineEdit> {
        std::mem::take(&mut self.changes.lines)
    }

    /// Cuántos reemplazos se hicieron desde que existe el buffer, también
    /// los ya sacados con [`Buffer::take_splices`]. Para quien sigue los
    /// reemplazos sin sacarlos, como el motor vim con las posiciones que
    /// guarda: anota este número y luego pide [`Buffer::splices_since`].
    #[must_use]
    pub fn splice_count(&self) -> usize {
        self.changes.taken + self.changes.splices.len()
    }

    /// Los reemplazos hechos desde que [`Buffer::splice_count`] era
    /// `count`, en orden y sin sacarlos. Los que ya sacó
    /// [`Buffer::take_splices`] no vuelven.
    #[must_use]
    pub fn splices_since(&self, count: usize) -> &[Splice] {
        let start = count.saturating_sub(self.changes.taken);
        self.changes.splices.get(start..).unwrap_or(&[])
    }

    /// Estado del historial en el que está el texto.
    #[must_use]
    pub fn history_seq(&self) -> HistorySeq {
        self.history.current()
    }

    /// El texto de ahora, sin copiarlo, para otro hilo.
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        Snapshot::new(self.rope.clone())
    }

    /// # Errors
    /// [`BufferError::TransactionAlreadyOpen`] si ya hay una abierta.
    pub fn begin_transaction(&mut self, before: Selections) -> Result<(), BufferError> {
        self.history.begin(before, self.anchors.clone())
    }

    /// Cierra la transacción abierta; si no tuvo ediciones no se guarda.
    ///
    /// # Errors
    /// [`BufferError::NoOpenTransaction`] si no hay ninguna abierta.
    pub fn commit_transaction(&mut self, after: Selections) -> Result<(), BufferError> {
        self.history.commit(after)
    }

    /// Las transacciones que se guarden hasta [`Buffer::end_undo_group`]
    /// forman un solo paso de undo, como los cambios de una macro de vim.
    /// Moverse por el historial corta el grupo. Un grupo abierto dentro de
    /// otro es parte del de afuera.
    pub fn begin_undo_group(&mut self) {
        self.history.begin_group();
    }

    /// Si hay una transacción abierta, el grupo termina cuando se guarde.
    pub fn end_undo_group(&mut self) {
        self.history.end_group();
    }

    #[must_use]
    pub fn has_open_transaction(&self) -> bool {
        self.history.is_open()
    }

    /// El texto ya no es el de [`Buffer::history_seq`]: la transacción
    /// abierta tiene ediciones.
    #[must_use]
    pub fn has_uncommitted_edits(&self) -> bool {
        self.history.has_uncommitted_edits()
    }

    /// Aplica un lote de ediciones sin solapes, expresadas en coordenadas del
    /// texto actual, dentro de la transacción abierta.
    ///
    /// # Errors
    /// Si no hay transacción abierta o alguna edición es inválida; en ese caso
    /// no se aplica ninguna.
    pub fn apply(&mut self, edits: Vec<Edit>) -> Result<EditMap, BufferError> {
        self.apply_following(edits, None)
    }

    /// Como [`Buffer::apply`]; con `moved`, las anclas y los [`Splice`]
    /// siguen a ese movimiento de líneas en vez de a cada reemplazo.
    fn apply_following(
        &mut self,
        edits: Vec<Edit>,
        moved: Option<LineMove>,
    ) -> Result<EditMap, BufferError> {
        self.history.ensure_open()?;
        let mut located = edits
            .into_iter()
            .map(|edit| Ok((self.spots(&edit.range)?, edit)))
            .collect::<Result<Vec<_>, BufferError>>()?;
        located.retain(|(_, edit)| !(edit.range.is_empty() && edit.text.is_empty()));
        located.sort_by_key(|(_, edit)| edit.range.start);
        let (spots, edits): (Vec<_>, Vec<_>) = located.into_iter().unzip();
        check_overlaps(&edits)?;
        let mut applied = Vec::with_capacity(edits.len());
        // De atrás hacia adelante, así lo de antes de cada edición, y con
        // eso sus `Spot`, no cambia.
        for (index, (&(start, end), edit)) in spots.iter().zip(&edits).rev().enumerate() {
            // Las anclas siguen al movimiento en cuanto empieza.
            let follow = moved.map(|moved| {
                if index == 0 {
                    LineChange::moved(moved)
                } else {
                    LineChange::UNCHANGED
                }
            });
            applied.push(AppliedEdit {
                start: edit.range.start,
                deleted: self.slice(start.byte..end.byte).into_owned(),
                inserted: edit.text.clone(),
            });
            splice(
                &mut self.rope,
                &mut self.anchors,
                &mut self.marked,
                &mut self.changes,
                (start, end),
                &edit.text,
                follow,
            )?;
            self.version += 1;
        }
        self.history.record(applied)?;
        Ok(EditMap::new(&edits))
    }

    /// Los dos extremos de `range`, con los errores de [`Self::check_range`].
    fn spots(&self, range: &Range<ByteOffset>) -> Result<(Spot, Spot), BufferError> {
        self.check_bounds(range)?;
        let spot = |offset: ByteOffset| {
            Spot::of(&self.rope, offset.get()).ok_or(BufferError::NotCharBoundary(offset))
        };
        let start = spot(range.start)?;
        let end = if range.is_empty() {
            start
        } else {
            spot(range.end)?
        };
        Ok((start, end))
    }

    fn check_range(&self, range: &Range<ByteOffset>) -> Result<(), BufferError> {
        self.check_bounds(range)?;
        for offset in [range.start, range.end] {
            if !self.is_char_boundary(offset) {
                return Err(BufferError::NotCharBoundary(offset));
            }
        }
        Ok(())
    }

    fn check_bounds(&self, range: &Range<ByteOffset>) -> Result<(), BufferError> {
        if range.start > range.end {
            return Err(BufferError::InvertedRange {
                start: range.start,
                end: range.end,
            });
        }
        if range.end > self.len() {
            return Err(BufferError::OutOfBounds {
                start: range.start,
                end: range.end,
                len: self.len(),
            });
        }
        Ok(())
    }
}

fn check_overlaps(sorted: &[Edit]) -> Result<(), BufferError> {
    for pair in sorted.windows(2) {
        if pair[0].range.end > pair[1].range.start {
            return Err(BufferError::OverlappingEdits {
                first: pair[0].range.clone(),
                second: pair[1].range.clone(),
            });
        }
    }
    Ok(())
}

/// Los extremos de los `removed` bytes desde `start`, de una edición del
/// historial.
fn locate(rope: &Rope, start: ByteOffset, removed: usize) -> Result<(Spot, Spot), BufferError> {
    let out_of_bounds = || BufferError::OutOfBounds {
        start,
        end: start + removed,
        len: ByteOffset::new(rope.len_bytes()),
    };
    let first = Spot::of(rope, start.get()).ok_or_else(out_of_bounds)?;
    let last = Spot::of(rope, start.get() + removed).ok_or_else(out_of_bounds)?;
    Ok((first, last))
}

/// Reemplaza `start..end` por `text`; lo que sigue a las líneas va como
/// diga `follow` o, si no, con el reemplazo.
fn splice(
    rope: &mut Rope,
    anchors: &mut Anchors,
    marked: &mut MarkedLines,
    log: &mut Changes,
    (start, end): (Spot, Spot),
    text: &str,
    follow: Option<LineChange>,
) -> Result<(), BufferError> {
    let removed = end.byte - start.byte;
    let len = ByteOffset::new(rope.len_bytes());
    let out_of_bounds = |_| BufferError::OutOfBounds {
        start: ByteOffset::new(start.byte),
        end: ByteOffset::new(end.byte),
        len,
    };
    let change = Replacement::between(rope, &start, &end, text);
    let follow = follow.unwrap_or(LineChange::replaced(change));
    anchors.adjust(&follow);
    marked.adjust(&change);
    // Un rango vacío también recorre el rope.
    if removed > 0 {
        rope.try_remove(start.char..end.char)
            .map_err(out_of_bounds)?;
    }
    if !text.is_empty() {
        rope.try_insert(start.char, text).map_err(out_of_bounds)?;
    }
    log.splices.push(Splice {
        start: ByteOffset::new(start.byte),
        removed,
        inserted: text.len(),
        lines: follow,
    });
    log.lines.push(change.line_edit());
    Ok(())
}

impl From<&str> for Buffer {
    fn from(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
            ..Self::default()
        }
    }
}

impl fmt::Display for Buffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for chunk in self.rope.chunks() {
            f.write_str(chunk)?;
        }
        Ok(())
    }
}
