//! La fila elegida de una lista de autocompletado y las que se ven.

use std::ops::Range;

use crate::row_cursor::RowCursor;

/// Sobre una lista de `len` filas, al menos una.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    cursor: RowCursor,
    /// Si la fila la eligió el usuario; si no, es la mejor coincidencia.
    chosen: bool,
    /// La primera fila que se ve.
    first: usize,
}

impl Selection {
    pub(super) fn new(len: usize) -> Self {
        Self {
            cursor: RowCursor::wrapping(len.max(1)),
            chosen: false,
            first: 0,
        }
    }

    #[must_use]
    pub fn selected(&self) -> usize {
        self.cursor.selected()
    }

    /// La elegida, solo si la eligió el usuario.
    pub(super) fn chosen(&self) -> Option<usize> {
        self.chosen.then_some(self.selected())
    }

    /// Elige `row` como lo hizo el usuario, con la ventana de `previous`:
    /// la misma lista tras un lote nuevo del vault.
    pub(super) fn keep(&mut self, row: usize, previous: &Self) {
        self.select(row);
        self.first = previous.first;
    }

    /// Las filas que se ven, `rows` como mucho. Como en un picker, solo se
    /// corren cuando la elegida sale de ellas.
    pub fn visible(&mut self, rows: usize) -> Range<usize> {
        let len = self.cursor.len();
        let rows = rows.clamp(1, len);
        let selected = self.selected();
        if selected < self.first {
            self.first = selected;
        } else if selected >= self.first + rows {
            self.first = selected + 1 - rows;
        }
        self.first = self.first.min(len - rows);
        self.first..self.first + rows
    }

    /// La siguiente, y tras la última la primera.
    pub fn select_next(&mut self) {
        self.select(self.cursor.next());
    }

    /// La anterior, y antes de la primera la última.
    pub fn select_previous(&mut self) {
        self.select(self.cursor.previous());
    }

    pub fn select(&mut self, row: usize) {
        self.cursor.select(row.min(self.cursor.len() - 1));
        self.chosen = true;
    }
}
