use std::ops::Range;

use crate::ByteOffset;

/// Reemplaza `range` por `text`. El rango se expresa en coordenadas del
/// texto anterior a todo el lote de ediciones.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    pub range: Range<ByteOffset>,
    pub text: String,
}

impl Edit {
    #[must_use]
    pub fn insert(at: ByteOffset, text: impl Into<String>) -> Self {
        Self {
            range: at..at,
            text: text.into(),
        }
    }

    #[must_use]
    pub fn delete(range: Range<ByteOffset>) -> Self {
        Self {
            range,
            text: String::new(),
        }
    }

    #[must_use]
    pub fn replace(range: Range<ByteOffset>, text: impl Into<String>) -> Self {
        Self {
            range,
            text: text.into(),
        }
    }
}

/// Hacia qué lado se va una posición que cae justo en una edición.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bias {
    /// Queda antes del texto insertado.
    Before,
    /// Queda después del texto insertado.
    After,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Change {
    start: usize,
    old_end: usize,
    new_len: usize,
}

/// Traduce posiciones del texto anterior a un lote de ediciones al texto
/// resultante.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EditMap {
    changes: Vec<Change>,
}

impl EditMap {
    /// `edits` debe venir ordenado y sin solapes.
    pub(crate) fn new(edits: &[Edit]) -> Self {
        let changes = edits
            .iter()
            .map(|edit| Change {
                start: edit.range.start.get(),
                old_end: edit.range.end.get(),
                new_len: edit.text.len(),
            })
            .collect();
        Self { changes }
    }

    #[must_use]
    pub fn map(&self, offset: ByteOffset, bias: Bias) -> ByteOffset {
        let offset = offset.get();
        let mut grown = 0;
        let mut shrunk = 0;
        for change in &self.changes {
            let past = match bias {
                Bias::Before => offset > change.old_end,
                Bias::After => offset >= change.old_end,
            };
            if past {
                grown += change.new_len;
                shrunk += change.old_end - change.start;
                continue;
            }
            if offset < change.start {
                break;
            }
            let start = change.start - shrunk + grown;
            return ByteOffset::new(match bias {
                Bias::Before => start,
                Bias::After => start + change.new_len,
            });
        }
        ByteOffset::new(offset - shrunk + grown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(offset: usize) -> ByteOffset {
        ByteOffset::new(offset)
    }

    fn map(edits: &[Edit], offset: usize, bias: Bias) -> usize {
        EditMap::new(edits).map(at(offset), bias).get()
    }

    #[test]
    fn shifts_positions_after_an_insertion() {
        let edits = [Edit::insert(at(2), "xyz")];
        assert_eq!(map(&edits, 1, Bias::After), 1);
        assert_eq!(map(&edits, 2, Bias::Before), 2);
        assert_eq!(map(&edits, 2, Bias::After), 5);
        assert_eq!(map(&edits, 4, Bias::Before), 7);
    }

    #[test]
    fn collapses_positions_inside_a_deletion() {
        let edits = [Edit::delete(at(2)..at(5))];
        assert_eq!(map(&edits, 3, Bias::Before), 2);
        assert_eq!(map(&edits, 5, Bias::Before), 2);
        assert_eq!(map(&edits, 6, Bias::After), 3);
    }

    #[test]
    fn accumulates_several_edits() {
        let edits = [
            Edit::delete(at(0)..at(2)),
            Edit::insert(at(4), "abc"),
            Edit::replace(at(6)..at(8), "z"),
        ];
        assert_eq!(map(&edits, 3, Bias::Before), 1);
        assert_eq!(map(&edits, 4, Bias::After), 5);
        assert_eq!(map(&edits, 9, Bias::Before), 9);
    }
}
