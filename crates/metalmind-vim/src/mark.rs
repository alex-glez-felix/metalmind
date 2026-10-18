//! Marks `a`–`z` y la del salto anterior (`''`), como anclas de línea del
//! buffer: siguen a su línea cuando se edita arriba y se pierden si se
//! borra.

use metalmind_buffer::{Buffer, BufferError, ByteOffset, LineAnchor};

use crate::lines::{clamp_normal, first_non_blank, last_char};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MarkName {
    /// `a`–`z`, por su índice.
    Named(u8),
    /// `'` o `` ` ``: dónde estaba el cursor antes del último salto.
    Previous,
}

impl MarkName {
    pub fn parse(c: char) -> Option<Self> {
        match c {
            'a'..='z' => Some(Self::Named(c as u8 - b'a')),
            '\'' | '`' => Some(Self::Previous),
            _ => None,
        }
    }
}

/// Las marks de un buffer. Son anclas de ese buffer, así que quien edita
/// varios con un solo [`Vim`](crate::Vim) le presta las del que edita con
/// [`Vim::swap_marks`](crate::Vim::swap_marks), como vim, donde son de cada
/// buffer.
#[derive(Debug, Default)]
pub struct Marks {
    named: [Option<LineAnchor>; 26],
    previous: Option<LineAnchor>,
}

impl Marks {
    pub(crate) fn set(
        &mut self,
        buffer: &mut Buffer,
        name: MarkName,
        offset: ByteOffset,
    ) -> Result<(), BufferError> {
        let slot = self.slot(name);
        if let Some(anchor) = *slot {
            return buffer.move_anchor(anchor, offset);
        }
        *slot = Some(buffer.create_anchor(offset)?);
        Ok(())
    }

    /// `None` si no se puso o se borró su línea.
    pub(crate) fn get(&self, buffer: &Buffer, name: MarkName) -> Option<ByteOffset> {
        let anchor = match name {
            MarkName::Named(letter) => self.named[usize::from(letter)],
            MarkName::Previous => self.previous,
        };
        buffer.anchor_offset(anchor?)
    }

    fn slot(&mut self, name: MarkName) -> &mut Option<LineAnchor> {
        match name {
            MarkName::Named(letter) => &mut self.named[usize::from(letter)],
            MarkName::Previous => &mut self.previous,
        }
    }
}

/// A dónde lleva una mark: su posición con `` ` ``, nunca sobre el fin de
/// una línea con texto, o el primer no blanco de su línea con `'`.
pub(crate) fn destination(buffer: &Buffer, offset: ByteOffset, exact: bool) -> ByteOffset {
    if exact {
        return clamp_normal(buffer, offset);
    }
    let line = buffer.line_of(offset);
    first_non_blank(buffer, line).min(last_char(buffer, line))
}
