use std::borrow::Cow;
use std::fmt;
use std::ops::Range;

use ropey::Rope;

mod anchored;
mod marked;

use crate::anchor::{Anchors, LineChange};
use crate::history::{AppliedEdit, Direction, History, Step};
use crate::marked::MarkedLines;
use crate::{
    BufferError, ByteOffset, Edit, EditMap, HistorySeq, LineEdit, Selections, Snapshot, Splice,
    Transaction,
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
        std::mem::take(&mut self.changes.splices)
    }

    /// Los reemplazos desde la última llamada, por líneas y en orden. Es
    /// otra lista que la de [`Buffer::take_splices`]: la lee otro (el
    /// estilo de las líneas) en otro momento.
    pub fn take_line_edits(&mut self) -> Vec<LineEdit> {
        std::mem::take(&mut self.changes.lines)
    }

    /// Los reemplazos hechos desde el último [`Buffer::take_splices`], sin
    /// sacarlos: para quien los sigue a medias, como el motor vim con las
    /// posiciones que guarda mientras edita.
    #[must_use]
    pub fn splices(&self) -> &[Splice] {
        &self.changes.splices
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

    #[must_use]
    pub fn len(&self) -> ByteOffset {
        ByteOffset::new(self.rope.len_bytes())
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rope.len_bytes() == 0
    }

    #[must_use]
    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    #[must_use]
    pub fn last_line(&self) -> usize {
        self.line_count() - 1
    }

    /// Línea que contiene `offset`; los offsets fuera del texto van a la última.
    #[must_use]
    pub fn line_of(&self, offset: ByteOffset) -> usize {
        self.rope.byte_to_line(offset.min(self.len()).get())
    }

    #[must_use]
    pub fn line_start(&self, line: usize) -> ByteOffset {
        ByteOffset::new(self.rope.line_to_byte(line.min(self.last_line())))
    }

    /// Offset del `\n` que cierra la línea, o el final del texto.
    #[must_use]
    pub fn line_end(&self, line: usize) -> ByteOffset {
        if line < self.last_line() {
            ByteOffset::new(self.rope.line_to_byte(line + 1) - 1)
        } else {
            self.len()
        }
    }

    #[must_use]
    pub fn is_char_boundary(&self, offset: ByteOffset) -> bool {
        let bytes = offset.get();
        bytes <= self.rope.len_bytes()
            && self.rope.char_to_byte(self.rope.byte_to_char(bytes)) == bytes
    }

    /// Carácter que empieza en `offset`.
    #[must_use]
    pub fn char_at(&self, offset: ByteOffset) -> Option<char> {
        if !self.is_char_boundary(offset) {
            return None;
        }
        self.rope.get_char(self.rope.byte_to_char(offset.get()))
    }

    /// Carácter que termina en `offset`.
    #[must_use]
    pub fn char_before(&self, offset: ByteOffset) -> Option<char> {
        if offset == ByteOffset::ZERO || !self.is_char_boundary(offset) {
            return None;
        }
        self.rope.get_char(self.rope.byte_to_char(offset.get()) - 1)
    }

    /// Inicio del carácter siguiente; el final del texto se queda donde está.
    #[must_use]
    pub fn next_boundary(&self, offset: ByteOffset) -> ByteOffset {
        self.char_at(offset)
            .map_or(offset, |c| offset + c.len_utf8())
    }

    /// Inicio del carácter anterior; el inicio del texto se queda donde está.
    #[must_use]
    pub fn prev_boundary(&self, offset: ByteOffset) -> ByteOffset {
        self.char_before(offset)
            .map_or(offset, |c| offset.saturating_sub(c.len_utf8()))
    }

    /// Caracteres entre el inicio de la línea y `offset`.
    #[must_use]
    pub fn column_of(&self, offset: ByteOffset) -> usize {
        let offset = offset.min(self.len()).get();
        let line = self.rope.byte_to_line(offset);
        self.rope.byte_to_char(offset) - self.rope.line_to_char(line)
    }

    /// Offset de la columna `column` (en caracteres), sin pasar del fin de línea.
    #[must_use]
    pub fn offset_at_column(&self, line: usize, column: usize) -> ByteOffset {
        let line_start = self.rope.byte_to_char(self.line_start(line).get());
        let line_end = self.rope.byte_to_char(self.line_end(line).get());
        let target = line_start.saturating_add(column).min(line_end);
        ByteOffset::new(self.rope.char_to_byte(target))
    }

    /// Texto de la línea sin su `\n`; sin copiar si está en un solo trozo del rope.
    #[must_use]
    pub fn line_text(&self, line: usize) -> Cow<'_, str> {
        self.lines_text(line..line + 1)
    }

    /// Texto seguido de las líneas `lines`, sin el `\n` de la última; vacío
    /// si el rango lo está. Sin copiar si está en un solo trozo del rope.
    #[must_use]
    pub fn lines_text(&self, lines: Range<usize>) -> Cow<'_, str> {
        if lines.is_empty() {
            return Cow::Borrowed("");
        }
        let start = self.line_start(lines.start).get();
        let end = self.line_end(lines.end - 1).get();
        let slice = self.rope.byte_slice(start..end);
        slice
            .as_str()
            .map_or_else(|| Cow::Owned(slice.to_string()), Cow::Borrowed)
    }

    /// # Errors
    /// Si el rango está invertido, se sale del texto o parte un carácter.
    pub fn text_in(&self, range: Range<ByteOffset>) -> Result<String, BufferError> {
        self.check_range(&range)?;
        Ok(self
            .rope
            .byte_slice(range.start.get()..range.end.get())
            .to_string())
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
    pub fn apply(&mut self, mut edits: Vec<Edit>) -> Result<EditMap, BufferError> {
        self.history.ensure_open()?;
        for edit in &edits {
            self.check_range(&edit.range)?;
        }
        edits.retain(|edit| !(edit.range.is_empty() && edit.text.is_empty()));
        edits.sort_by_key(|edit| edit.range.start);
        check_overlaps(&edits)?;
        let mut applied = Vec::with_capacity(edits.len());
        for edit in edits.iter().rev() {
            let deleted = self.text_in(edit.range.clone())?;
            let removed = deleted.len();
            applied.push(AppliedEdit {
                start: edit.range.start,
                deleted,
                inserted: edit.text.clone(),
            });
            splice(
                &mut self.rope,
                &mut self.anchors,
                &mut self.marked,
                &mut self.changes,
                edit.range.start,
                removed,
                &edit.text,
            )?;
            self.version += 1;
        }
        self.history.record(applied)?;
        Ok(EditMap::new(&edits))
    }

    /// Vuelve al estado anterior en la rama actual y devuelve la transacción
    /// deshecha.
    ///
    /// # Errors
    /// [`BufferError::TransactionAlreadyOpen`] si hay una transacción abierta.
    pub fn undo(&mut self) -> Result<Option<&Transaction>, BufferError> {
        let steps = self.history.undo()?;
        self.travel(&steps)
    }

    /// Rehace la transacción deshecha o visitada más recientemente desde el
    /// estado actual y la devuelve.
    ///
    /// # Errors
    /// [`BufferError::TransactionAlreadyOpen`] si hay una transacción abierta.
    pub fn redo(&mut self) -> Result<Option<&Transaction>, BufferError> {
        let steps = self.history.redo()?;
        self.travel(&steps)
    }

    /// Retrocede `steps` estados en orden cronológico, aunque estén en otra
    /// rama, como `g-` en vim. Devuelve la última transacción recorrida.
    ///
    /// # Errors
    /// [`BufferError::TransactionAlreadyOpen`] si hay una transacción abierta.
    pub fn earlier(&mut self, steps: usize) -> Result<Option<&Transaction>, BufferError> {
        let steps = self.history.earlier(steps)?;
        self.travel(&steps)
    }

    /// Avanza `steps` estados en orden cronológico, aunque estén en otra
    /// rama, como `g+` en vim. Devuelve la última transacción recorrida.
    ///
    /// # Errors
    /// [`BufferError::TransactionAlreadyOpen`] si hay una transacción abierta.
    pub fn later(&mut self, steps: usize) -> Result<Option<&Transaction>, BufferError> {
        let steps = self.history.later(steps)?;
        self.travel(&steps)
    }

    /// Deshace o rehace cada transacción del recorrido y devuelve la última.
    /// Como en vim, cada una deja las anclas donde estaban del otro lado.
    fn travel(&mut self, steps: &[Step]) -> Result<Option<&Transaction>, BufferError> {
        for step in steps {
            let Some(transaction) = self.history.transaction(step.seq) else {
                continue;
            };
            let anchors_before = self.anchors.clone();
            let edits = transaction.edits();
            match step.direction {
                Direction::Undo => {
                    for edit in edits.iter().rev() {
                        let removed = edit.inserted.len();
                        splice(
                            &mut self.rope,
                            &mut self.anchors,
                            &mut self.marked,
                            &mut self.changes,
                            edit.start,
                            removed,
                            &edit.deleted,
                        )?;
                    }
                }
                Direction::Redo => {
                    for edit in edits {
                        let removed = edit.deleted.len();
                        splice(
                            &mut self.rope,
                            &mut self.anchors,
                            &mut self.marked,
                            &mut self.changes,
                            edit.start,
                            removed,
                            &edit.inserted,
                        )?;
                    }
                }
            }
            self.version += 1;
            let saved = self.history.swap_anchors(step.seq, anchors_before);
            self.anchors.restore(&saved);
        }
        Ok(steps
            .last()
            .and_then(|step| self.history.transaction(step.seq)))
    }

    fn check_range(&self, range: &Range<ByteOffset>) -> Result<(), BufferError> {
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
        for offset in [range.start, range.end] {
            if !self.is_char_boundary(offset) {
                return Err(BufferError::NotCharBoundary(offset));
            }
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

fn splice(
    rope: &mut Rope,
    anchors: &mut Anchors,
    marked: &mut MarkedLines,
    log: &mut Changes,
    start: ByteOffset,
    removed: usize,
    text: &str,
) -> Result<(), BufferError> {
    let len = ByteOffset::new(rope.len_bytes());
    let out_of_bounds = |_| BufferError::OutOfBounds {
        start,
        end: start + removed,
        len,
    };
    let first = rope.try_byte_to_char(start.get()).map_err(out_of_bounds)?;
    let last = rope
        .try_byte_to_char(start.get() + removed)
        .map_err(out_of_bounds)?;
    let change = LineChange::new(rope, start.get(), removed, text);
    anchors.adjust(&change);
    marked.adjust(&change);
    rope.try_remove(first..last).map_err(out_of_bounds)?;
    rope.try_insert(first, text).map_err(out_of_bounds)?;
    log.splices.push(Splice {
        start,
        removed,
        inserted: text.len(),
        lines: change,
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
