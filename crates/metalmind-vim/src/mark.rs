//! Marks `a`–`z` y la del salto anterior (`''`), como anclas de línea del
//! buffer: siguen a su línea cuando se edita arriba y se pierden si se
//! borra. Las de archivo, `A`–`Z`, son del motor (ver [`FileMarks`]).

use metalmind_buffer::{Buffer, BufferError, ByteOffset, LineAnchor, Splice};

use crate::jumplist::{FileId, Jump};
use crate::lines::{clamp_normal, first_non_blank, last_char};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MarkName {
    /// `a`–`z`, por su índice.
    Named(u8),
    /// `'` o `` ` ``: dónde estaba el cursor antes del último salto.
    Previous,
    /// `A`–`Z`, por su índice: un archivo y una posición en él.
    File(u8),
}

impl MarkName {
    pub fn parse(c: char) -> Option<Self> {
        match c {
            'a'..='z' => Some(Self::Named(c as u8 - b'a')),
            'A'..='Z' => Some(Self::File(c as u8 - b'A')),
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
        name: BufferMark,
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
    pub(crate) fn get(&self, buffer: &Buffer, name: BufferMark) -> Option<ByteOffset> {
        let anchor = match name {
            BufferMark::Named(letter) => self.named[usize::from(letter)],
            BufferMark::Previous => self.previous,
        };
        buffer.anchor_offset(anchor?)
    }

    fn slot(&mut self, name: BufferMark) -> &mut Option<LineAnchor> {
        match name {
            BufferMark::Named(letter) => &mut self.named[usize::from(letter)],
            BufferMark::Previous => &mut self.previous,
        }
    }
}

/// Las marks de [`Marks`]: las que son de cada buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BufferMark {
    Named(u8),
    Previous,
}

impl MarkName {
    /// `None` si es una mark de archivo.
    pub(crate) fn of_buffer(self) -> Option<BufferMark> {
        match self {
            Self::Named(letter) => Some(BufferMark::Named(letter)),
            Self::Previous => Some(BufferMark::Previous),
            Self::File(_) => None,
        }
    }
}

/// Las marks `A`–`Z`: no son de un buffer sino de un archivo, así que valen
/// desde cualquiera, como en vim. Siguen a su línea mientras se edita su
/// archivo y, si se borra, quedan donde empezó lo borrado
/// (`one_adjust_nodel`); se pierden si se borra el archivo.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct FileMarks([Option<Jump>; 26]);

impl FileMarks {
    pub fn set(&mut self, letter: u8, jump: Jump) {
        self.0[usize::from(letter)] = Some(jump);
    }

    pub fn get(&self, letter: u8) -> Option<Jump> {
        self.0[usize::from(letter)]
    }

    pub fn adjust(&mut self, file: FileId, splices: &[Splice]) {
        for slot in &mut self.0 {
            if let Some(jump) = slot.as_mut().filter(|jump| jump.file == file) {
                jump.point = splices
                    .iter()
                    .fold(jump.point, |point, splice| splice.lines.keep(point));
            }
        }
    }

    /// El archivo ya no existe: sus marks se pierden.
    pub fn forget(&mut self, file: FileId) {
        for slot in &mut self.0 {
            if slot.is_some_and(|jump| jump.file == file) {
                *slot = None;
            }
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
