//! Text objects con la lógica de `current_word`, `current_block`,
//! `current_quote` y `current_par` de vim (`textobject.c`), incluida la de
//! Visual, donde extienden la selección.

mod block;
mod paragraph;
mod quote;
mod word;

use metalmind_buffer::{Buffer, ByteOffset, Goal, Selection};

use crate::VisualKind;
use crate::span::{Missed, Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextObject {
    Word { big: bool, around: bool },
    Parentheses { around: bool },
    DoubleQuotes { around: bool },
    Paragraph { around: bool },
}

/// Los extremos que mueve un text object, como `VIsual` y el cursor de vim.
/// Con un operador pendiente, `anchor` hace de inicio del objeto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Ends {
    anchor: ByteOffset,
    head: ByteOffset,
}

impl Ends {
    fn at(head: ByteOffset) -> Self {
        Self { anchor: head, head }
    }
}

impl TextObject {
    /// Lo que abarca con un operador pendiente; falla si no hay objeto
    /// alrededor del cursor.
    pub(crate) fn resolve(
        self,
        buffer: &Buffer,
        head: ByteOffset,
        count: usize,
    ) -> Result<Span, Missed> {
        let missed = || Missed::at(buffer, head);
        match self {
            Self::Word { big, around } => word::resolve(buffer, head, count, big, around),
            Self::Parentheses { around } => {
                block::resolve(buffer, head, count, around, ('(', ')')).ok_or_else(missed)
            }
            Self::DoubleQuotes { around } => {
                quote::resolve(buffer, head, count, around, '"').ok_or_else(missed)
            }
            Self::Paragraph { around } => {
                paragraph::resolve(buffer, head, count, around).ok_or_else(missed)
            }
        }
    }

    /// En Visual, selecciona el objeto o extiende la selección con él. Como
    /// en vim, si falla la selección puede quedar a medio mover; el modo
    /// pasa a ser por caracteres, o por líneas con `ip` y `ap`. Un bloque
    /// sigue siéndolo con las palabras y las comillas.
    pub(crate) fn select(
        self,
        buffer: &Buffer,
        selection: &Selection,
        kind: VisualKind,
        count: usize,
    ) -> (Selection, VisualKind) {
        let mut ends = Ends {
            anchor: selection.anchor,
            head: selection.head,
        };
        let by_chars = |found: bool| if found { VisualKind::Char } else { kind };
        let keeps_block = |found: bool| match kind {
            VisualKind::Block => VisualKind::Block,
            _ => by_chars(found),
        };
        let kind = match self {
            Self::Word { big, around } => {
                keeps_block(word::select(buffer, &mut ends, count, big, around))
            }
            Self::Parentheses { around } => {
                by_chars(block::select(buffer, &mut ends, count, around, ('(', ')')))
            }
            Self::DoubleQuotes { around } => {
                keeps_block(quote::select(buffer, &mut ends, count, around, '"'))
            }
            Self::Paragraph { around } => paragraph::select(buffer, &mut ends, kind, count, around),
        };
        let moved = ends.anchor != selection.anchor || ends.head != selection.head;
        let selection = Selection {
            anchor: ends.anchor,
            head: ends.head,
            goal: if moved { Goal::None } else { selection.goal },
        };
        (selection, kind)
    }
}
