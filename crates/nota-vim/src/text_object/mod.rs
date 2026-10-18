//! Text objects con la lógica de `current_word`, `current_block`,
//! `current_quote` y `current_par` de vim (`textobject.c`).

mod block;
mod paragraph;
mod quote;
mod word;

use nota_buffer::{Buffer, ByteOffset};

use crate::span::{Missed, Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextObject {
    Word { big: bool, around: bool },
    Parentheses { around: bool },
    DoubleQuotes { around: bool },
    Paragraph { around: bool },
}

impl TextObject {
    /// Falla si no hay objeto alrededor del cursor.
    pub(crate) fn resolve(
        self,
        buffer: &Buffer,
        head: ByteOffset,
        count: usize,
    ) -> Result<Span, Missed> {
        let missed = || Missed::at(buffer, head);
        match self {
            Self::Word { big, around } => word::select(buffer, head, count, big, around),
            Self::Parentheses { around } => {
                block::select(buffer, head, count, around, ('(', ')')).ok_or_else(missed)
            }
            Self::DoubleQuotes { around } => {
                quote::select(buffer, head, count, around, '"').ok_or_else(missed)
            }
            Self::Paragraph { around } => {
                paragraph::select(buffer, head, count, around).ok_or_else(missed)
            }
        }
    }
}
