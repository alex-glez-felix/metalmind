use crate::ByteOffset;

/// Un reemplazo ya hecho en el texto: los `removed` bytes desde `start`
/// pasaron a ser `inserted` bytes. Sirve para mover posiciones guardadas
/// fuera del motor, como el cursor de otro panel sobre el mismo buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Splice {
    pub start: ByteOffset,
    pub removed: usize,
    pub inserted: usize,
}

impl Splice {
    /// Lo de antes no se mueve, ni lo que está justo donde se insertó; lo de
    /// después se corre y lo borrado cae al inicio del reemplazo.
    #[must_use]
    pub fn map(&self, offset: ByteOffset) -> ByteOffset {
        let start = self.start.get();
        let offset = offset.get();
        let old_end = start + self.removed;
        ByteOffset::new(if offset <= start {
            offset
        } else if offset >= old_end {
            offset - self.removed + self.inserted
        } else {
            start
        })
    }

    /// Aplica en orden los reemplazos de `splices`.
    #[must_use]
    pub fn map_all(splices: &[Self], offset: ByteOffset) -> ByteOffset {
        splices
            .iter()
            .fold(offset, |offset, splice| splice.map(offset))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn splice(start: usize, removed: usize, inserted: usize) -> Splice {
        Splice {
            start: ByteOffset::new(start),
            removed,
            inserted,
        }
    }

    #[test]
    fn mueve_lo_de_despues_y_colapsa_lo_borrado() {
        let cases = [
            (splice(2, 0, 3), 1, 1),
            (splice(2, 0, 3), 2, 2),
            (splice(2, 0, 3), 4, 7),
            (splice(2, 3, 0), 3, 2),
            (splice(2, 3, 0), 5, 2),
            (splice(2, 3, 1), 6, 4),
        ];
        for (splice, offset, expected) in cases {
            assert_eq!(
                splice.map(ByteOffset::new(offset)),
                ByteOffset::new(expected),
                "{splice:?} {offset}"
            );
        }
    }
}
