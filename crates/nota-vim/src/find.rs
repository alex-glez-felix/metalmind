//! `f`, `F`, `t`, `T`, `;` y `,` con la lógica de `searchc()` de vim.

use nota_buffer::{Buffer, ByteOffset};

use crate::motion::Destination;
use crate::span::SpanKind;

/// Cómo busca un carácter en la línea: hacia dónde y si se queda antes de él.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CharSearch {
    pub backward: bool,
    /// `t`/`T`: se detiene justo antes del carácter.
    pub till: bool,
}

/// Una búsqueda ya con su carácter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Find {
    search: CharSearch,
    target: char,
    /// Viene de `;` o `,`: un `t` repetido no se queda junto al mismo carácter.
    repeated: bool,
}

impl Find {
    pub fn new(search: CharSearch, target: char) -> Self {
        Self {
            search,
            target,
            repeated: false,
        }
    }

    /// La búsqueda de `;`, o la de `,` si `reversed`.
    pub fn repeat(self, reversed: bool) -> Self {
        Self {
            search: CharSearch {
                backward: self.search.backward != reversed,
                ..self.search
            },
            repeated: true,
            ..self
        }
    }

    /// `None` si no hay `count` apariciones en lo que queda de la línea.
    pub fn resolve(self, buffer: &Buffer, head: ByteOffset, count: usize) -> Option<Destination> {
        let CharSearch { backward, till } = self.search;
        let line = buffer.line_of(head);
        let (start, end) = (buffer.line_start(line), buffer.line_end(line));
        // Como `cpoptions` sin `;`: solo con count 1, como hace vim.
        let mut skip_adjacent = self.repeated && till && count == 1;
        let mut offset = head;
        for _ in 0..count {
            loop {
                offset = if backward {
                    (offset > start).then(|| buffer.prev_boundary(offset))?
                } else {
                    Some(buffer.next_boundary(offset)).filter(|&next| next < end)?
                };
                if buffer.char_at(offset) == Some(self.target) && !skip_adjacent {
                    break;
                }
                skip_adjacent = false;
            }
        }
        if till {
            offset = if backward {
                buffer.next_boundary(offset)
            } else {
                buffer.prev_boundary(offset)
            };
        }
        let kind = if backward {
            SpanKind::Exclusive
        } else {
            SpanKind::Inclusive
        };
        Some(Destination::new(offset, kind))
    }
}
