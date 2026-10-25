//! Líneas enteras: borrarlas, meterlas y moverlas, como los comandos de
//! líneas de vim.

use std::ops::RangeInclusive;

use super::Buffer;
use crate::line_move::LineMove;
use crate::{BufferError, ByteOffset, Edit};

impl Buffer {
    /// Quita `lines` con un salto de línea: el de después o, si son las
    /// últimas, el de antes.
    #[must_use]
    pub fn lines_deletion(&self, lines: &RangeInclusive<usize>) -> Edit {
        let (first, last) = (*lines.start(), *lines.end());
        Edit::delete(if last < self.last_line() {
            self.line_start(first)..self.line_start(last + 1)
        } else if first > 0 {
            self.line_end(first - 1)..self.len()
        } else {
            ByteOffset::new(0)..self.len()
        })
    }

    /// Mete `text`, líneas sin el salto final, debajo de la línea `below`
    /// contada desde 1; 0 es arriba de todo.
    #[must_use]
    pub fn lines_insertion(&self, below: usize, text: &str) -> Edit {
        if below < self.line_count() {
            Edit::insert(self.line_start(below), format!("{text}\n"))
        } else {
            Edit::insert(self.len(), format!("\n{text}"))
        }
    }

    /// Lleva `lines` debajo de la línea `below`, contada desde 1 (0 es
    /// arriba de todo), dentro de la transacción abierta. Como `:m` de vim,
    /// las anclas y lo que siga a los [`Splice`](crate::Splice) va con sus
    /// líneas; las marcadas, que vim copia, pierden la marca.
    ///
    /// # Errors
    /// [`BufferError::InvalidMove`] si alguna línea no está en el texto o
    /// `below` cae entre ellas, y los de [`Buffer::apply`].
    pub fn move_lines(
        &mut self,
        lines: RangeInclusive<usize>,
        below: usize,
    ) -> Result<(), BufferError> {
        let (first, last) = (*lines.start(), *lines.end());
        if first > last
            || last > self.last_line()
            || below > self.line_count()
            || (first + 1..=last).contains(&below)
        {
            return Err(BufferError::InvalidMove { first, last, below });
        }
        self.history.ensure_open()?;
        let Some(line_move) = LineMove::new(&lines, below) else {
            return Ok(());
        };
        // Primero la inserción: si cae justo donde empieza el borrado (las
        // últimas líneas suben sobre una vacía), va antes de él.
        let edits = vec![
            self.lines_insertion(below, &self.lines_text(first..last + 1)),
            self.lines_deletion(&lines),
        ];
        self.apply_following(edits, Some(line_move)).map(drop)
    }
}
