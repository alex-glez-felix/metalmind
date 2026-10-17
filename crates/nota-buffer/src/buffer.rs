use std::fmt;
use std::ops::Range;

use ropey::Rope;

use crate::history::{AppliedEdit, Direction, History, Step};
use crate::{BufferError, ByteOffset, Edit, EditMap, HistorySeq, Selections, Transaction};

/// Texto editable. Las líneas se separan solo por `\n`.
#[derive(Debug, Default)]
pub struct Buffer {
    rope: Rope,
    history: History,
    version: u64,
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

    /// Estado del historial en el que está el texto.
    #[must_use]
    pub fn history_seq(&self) -> HistorySeq {
        self.history.current()
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
        self.history.begin(before)
    }

    /// Cierra la transacción abierta; si no tuvo ediciones no se guarda.
    ///
    /// # Errors
    /// [`BufferError::NoOpenTransaction`] si no hay ninguna abierta.
    pub fn commit_transaction(&mut self, after: Selections) -> Result<(), BufferError> {
        self.history.commit(after)
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
        edits.retain(|edit| !(edit.range.is_empty() && edit.text.is_empty()));
        edits.sort_by_key(|edit| edit.range.start);
        self.check_edits(&edits)?;
        let mut applied = Vec::with_capacity(edits.len());
        for edit in edits.iter().rev() {
            let deleted = self.text_in(edit.range.clone())?;
            let removed = deleted.len();
            applied.push(AppliedEdit {
                start: edit.range.start,
                deleted,
                inserted: edit.text.clone(),
            });
            splice(&mut self.rope, edit.range.start, removed, &edit.text)?;
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
        replay(&mut self.rope, &mut self.version, &steps)
    }

    /// Rehace la transacción deshecha o visitada más recientemente desde el
    /// estado actual y la devuelve.
    ///
    /// # Errors
    /// [`BufferError::TransactionAlreadyOpen`] si hay una transacción abierta.
    pub fn redo(&mut self) -> Result<Option<&Transaction>, BufferError> {
        let steps = self.history.redo()?;
        replay(&mut self.rope, &mut self.version, &steps)
    }

    /// Retrocede `steps` estados en orden cronológico, aunque estén en otra
    /// rama, como `g-` en vim. Devuelve la última transacción recorrida.
    ///
    /// # Errors
    /// [`BufferError::TransactionAlreadyOpen`] si hay una transacción abierta.
    pub fn earlier(&mut self, steps: usize) -> Result<Option<&Transaction>, BufferError> {
        let steps = self.history.earlier(steps)?;
        replay(&mut self.rope, &mut self.version, &steps)
    }

    /// Avanza `steps` estados en orden cronológico, aunque estén en otra
    /// rama, como `g+` en vim. Devuelve la última transacción recorrida.
    ///
    /// # Errors
    /// [`BufferError::TransactionAlreadyOpen`] si hay una transacción abierta.
    pub fn later(&mut self, steps: usize) -> Result<Option<&Transaction>, BufferError> {
        let steps = self.history.later(steps)?;
        replay(&mut self.rope, &mut self.version, &steps)
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

    fn check_edits(&self, sorted: &[Edit]) -> Result<(), BufferError> {
        for edit in sorted {
            self.check_range(&edit.range)?;
        }
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
}

/// Deshace o rehace cada transacción del recorrido y devuelve la última.
fn replay<'a>(
    rope: &mut Rope,
    version: &mut u64,
    steps: &[Step<'a>],
) -> Result<Option<&'a Transaction>, BufferError> {
    for step in steps {
        let edits = step.transaction.edits();
        match step.direction {
            Direction::Undo => {
                for edit in edits.iter().rev() {
                    splice(rope, edit.start, edit.inserted.len(), &edit.deleted)?;
                }
            }
            Direction::Redo => {
                for edit in edits {
                    splice(rope, edit.start, edit.deleted.len(), &edit.inserted)?;
                }
            }
        }
        *version += 1;
    }
    Ok(steps.last().map(|step| step.transaction))
}

fn splice(
    rope: &mut Rope,
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
    rope.try_remove(first..last).map_err(out_of_bounds)?;
    rope.try_insert(first, text).map_err(out_of_bounds)?;
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
