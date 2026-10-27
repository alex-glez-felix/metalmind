//! La fila elegida de una lista de autocompletado y las que se ven.

use std::ops::Range;

/// Sobre una lista de `len` filas, al menos una.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    len: usize,
    selected: usize,
    /// Si `selected` la eligió el usuario; si no, es la mejor coincidencia.
    chosen: bool,
    /// La primera fila que se ve.
    first: usize,
}

impl Selection {
    pub(super) fn new(len: usize) -> Self {
        Self {
            len: len.max(1),
            selected: 0,
            chosen: false,
            first: 0,
        }
    }

    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// La elegida, solo si la eligió el usuario.
    pub(super) fn chosen(&self) -> Option<usize> {
        self.chosen.then_some(self.selected)
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
        let rows = rows.clamp(1, self.len);
        if self.selected < self.first {
            self.first = self.selected;
        } else if self.selected >= self.first + rows {
            self.first = self.selected + 1 - rows;
        }
        self.first = self.first.min(self.len - rows);
        self.first..self.first + rows
    }

    /// La siguiente, y tras la última la primera.
    pub fn select_next(&mut self) {
        self.selected = (self.selected + 1) % self.len;
        self.chosen = true;
    }

    /// La anterior, y antes de la primera la última.
    pub fn select_previous(&mut self) {
        self.selected = self.selected.checked_sub(1).unwrap_or(self.len - 1);
        self.chosen = true;
    }

    pub fn select(&mut self, row: usize) {
        self.selected = row.min(self.len - 1);
        self.chosen = true;
    }
}
