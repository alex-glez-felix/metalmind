//! La fila elegida de una lista, y qué pasa al moverse más allá de un borde.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AtEdge {
    Wrap,
    Stop,
}

/// `selected` siempre es una fila de la lista; vacía, es 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowCursor {
    selected: usize,
    len: usize,
    at_edge: AtEdge,
}

impl RowCursor {
    /// De la última fila se pasa a la primera, y al revés.
    #[must_use]
    pub fn wrapping(len: usize) -> Self {
        Self {
            selected: 0,
            len,
            at_edge: AtEdge::Wrap,
        }
    }

    /// Moverse más allá de un borde no hace nada.
    #[must_use]
    pub fn stopping(len: usize) -> Self {
        Self {
            at_edge: AtEdge::Stop,
            ..Self::wrapping(len)
        }
    }

    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Elige `row`, si existe.
    pub fn select(&mut self, row: usize) {
        if row < self.len {
            self.selected = row;
        }
    }

    /// La lista cambió de largo: si la fila elegida ya no está, queda la
    /// última.
    pub fn set_len(&mut self, len: usize) {
        self.len = len;
        self.selected = self.selected.min(len.saturating_sub(1));
    }

    /// La fila que sigue a la elegida.
    #[must_use]
    pub fn next(&self) -> usize {
        let last = self.len.saturating_sub(1);
        match self.at_edge {
            AtEdge::Wrap if self.selected == last => 0,
            AtEdge::Wrap | AtEdge::Stop => (self.selected + 1).min(last),
        }
    }

    /// La fila que antecede a la elegida.
    #[must_use]
    pub fn previous(&self) -> usize {
        match (self.selected.checked_sub(1), self.at_edge) {
            (Some(row), _) => row,
            (None, AtEdge::Wrap) => self.len.saturating_sub(1),
            (None, AtEdge::Stop) => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn at(mut cursor: RowCursor, row: usize) -> RowCursor {
        cursor.select(row);
        cursor
    }

    #[rstest]
    #[case::en_medio(1, 2, 0)]
    #[case::en_la_primera(0, 1, 2)]
    #[case::en_la_ultima(2, 0, 1)]
    fn dando_la_vuelta(#[case] row: usize, #[case] next: usize, #[case] previous: usize) {
        let cursor = at(RowCursor::wrapping(3), row);
        assert_eq!((cursor.next(), cursor.previous()), (next, previous));
    }

    #[rstest]
    #[case::en_medio(1, 2, 0)]
    #[case::en_la_primera(0, 1, 0)]
    #[case::en_la_ultima(2, 2, 1)]
    fn parandose(#[case] row: usize, #[case] next: usize, #[case] previous: usize) {
        let cursor = at(RowCursor::stopping(3), row);
        assert_eq!((cursor.next(), cursor.previous()), (next, previous));
    }

    #[rstest]
    #[case::vacia_da_la_vuelta(RowCursor::wrapping(0))]
    #[case::vacia_se_para(RowCursor::stopping(0))]
    #[case::una_da_la_vuelta(RowCursor::wrapping(1))]
    #[case::una_se_para(RowCursor::stopping(1))]
    fn sin_a_donde_ir_se_queda_en_la_cero(#[case] cursor: RowCursor) {
        assert_eq!((cursor.next(), cursor.previous()), (0, 0));
    }

    #[test]
    fn una_fila_que_no_existe_no_se_elige() {
        let mut cursor = at(RowCursor::wrapping(3), 2);
        cursor.select(3);
        assert_eq!(cursor.selected(), 2);
        let mut empty = RowCursor::stopping(0);
        empty.select(0);
        assert_eq!(empty.selected(), 0);
    }

    #[test]
    fn al_encogerse_la_lista_queda_la_ultima_fila() {
        let mut cursor = at(RowCursor::wrapping(5), 4);
        cursor.set_len(2);
        assert_eq!((cursor.selected(), cursor.len()), (1, 2));
        cursor.set_len(0);
        assert_eq!((cursor.selected(), cursor.len()), (0, 0));
        cursor.set_len(5);
        assert_eq!(cursor.selected(), 0, "crecer no mueve la fila elegida");
    }
}
