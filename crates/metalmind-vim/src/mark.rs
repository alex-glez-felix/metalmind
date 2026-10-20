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

/// Una mark puesta, para mostrarla: su letra y su línea, desde 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlacedMark {
    pub name: char,
    pub line: usize,
}

/// Las marks de un buffer, como en vim, donde son de cada buffer: son anclas
/// suyas y van en su [`DocState`](crate::DocState).
#[derive(Debug, Default)]
pub(crate) struct Marks {
    named: [Option<LineAnchor>; 26],
    previous: Option<LineAnchor>,
}

impl Marks {
    /// Las mismas marks, en la misma línea y columna, sobre `to`: el texto
    /// releído de `from`, como hace vim al releer un archivo. Las de una
    /// línea que `to` ya no tiene se pierden (vim las guarda y da `E19`).
    ///
    /// # Errors
    /// Solo si una mark no cae en `from`, lo que sería un bug.
    pub(crate) fn carried(&self, from: &Buffer, to: &mut Buffer) -> Result<Self, BufferError> {
        let mut carry = |anchor: Option<LineAnchor>| -> Result<Option<LineAnchor>, BufferError> {
            let Some(offset) = anchor.and_then(|anchor| from.anchor_offset(anchor)) else {
                return Ok(None);
            };
            let point = from.point_at(offset)?;
            if point.line > to.last_line() {
                return Ok(None);
            }
            to.create_anchor(to.offset_at_point(point)).map(Some)
        };
        let mut named = [None; 26];
        for (slot, &anchor) in named.iter_mut().zip(&self.named) {
            *slot = carry(anchor)?;
        }
        Ok(Self {
            named,
            previous: carry(self.previous)?,
        })
    }

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

    /// Borra la mark `a`–`z` de índice `letter`, como `:delm`: el undo de
    /// un cambio de cuando estaba puesta la devuelve, como en vim.
    pub(crate) fn delete(&mut self, buffer: &mut Buffer, letter: u8) -> Result<(), BufferError> {
        match self.named[usize::from(letter)] {
            Some(anchor) => buffer.clear_anchor(anchor),
            None => Ok(()),
        }
    }

    /// Las marks `a`–`z` puestas en `buffer`, en orden.
    pub(crate) fn named<'a>(
        &'a self,
        buffer: &'a Buffer,
    ) -> impl Iterator<Item = (u8, ByteOffset)> + 'a {
        (0..)
            .zip(&self.named)
            .filter_map(|(letter, anchor)| Some((letter, buffer.anchor_offset((*anchor)?)?)))
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

    pub fn delete(&mut self, letter: u8) {
        self.0[usize::from(letter)] = None;
    }

    /// Las de `file`, en orden.
    pub fn in_file(&self, file: FileId) -> impl Iterator<Item = (u8, Jump)> + '_ {
        (0..)
            .zip(&self.0)
            .filter_map(move |(letter, jump)| Some((letter, (*jump)?)))
            .filter(move |(_, jump)| jump.file == file)
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
