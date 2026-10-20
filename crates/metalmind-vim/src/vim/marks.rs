//! `m{a-zA-Z}`, `'{a-zA-Z}` y `` `{a-zA-Z} ``, y `''` y ` `` ` para volver.
//! Las `A`–`Z` llevan también el archivo y valen desde cualquier buffer.

use metalmind_buffer::{Buffer, ByteOffset, Goal, Point, Selection, Selections};

use super::Vim;
use crate::dot::{MarkJump, Subject};
use crate::jumplist::{FileId, Jump, Landing};
use crate::mark::{self, MarkName, PlacedMark};
use crate::object::Object;
use crate::{CommandError, DocState, KeyOutcome, Mode, Target, VimError};

impl Vim {
    /// `m{a-zA-Z}` en el cursor principal; `m'` pone la del salto anterior.
    pub(super) fn set_mark(&mut self, c: char, t: &mut Target<'_>) -> Result<(), VimError> {
        self.reset();
        let head = t.selections.primary().head;
        match MarkName::parse(c) {
            Some(MarkName::File(letter)) => {
                self.catch_up(t);
                let point = t.buffer.point_at(head)?;
                self.file_marks.set(
                    letter,
                    Jump {
                        file: t.file,
                        point,
                    },
                );
            }
            Some(name) => {
                if let Some(name) = name.of_buffer() {
                    t.doc.marks.set(t.buffer, name, head)?;
                }
            }
            None => self.fail(),
        }
        Ok(())
    }

    /// Las marks `a`–`z` de `doc` y las `A`–`Z` de `file` que caen en
    /// `buffer`, en ese orden.
    pub fn placed_marks<'a>(
        &'a self,
        doc: &'a DocState,
        file: FileId,
        buffer: &'a Buffer,
    ) -> impl Iterator<Item = PlacedMark> + 'a {
        let named = doc.marks.named(buffer).map(|(letter, offset)| PlacedMark {
            name: char::from(b'a' + letter),
            line: buffer.line_of(offset),
        });
        let of_file = self
            .file_marks
            .in_file(file)
            .map(move |(letter, jump)| PlacedMark {
                name: char::from(b'A' + letter),
                line: followed(jump.point, doc, buffer)
                    .line
                    .min(buffer.last_line()),
            });
        named.chain(of_file)
    }

    /// Dónde está la mark en este buffer; `None` si no se puso, se borró su
    /// línea o es de otro archivo.
    pub(super) fn mark_offset(&self, name: MarkName, t: &Target<'_>) -> Option<ByteOffset> {
        match name {
            MarkName::File(letter) => {
                let jump = self.file_marks.get(letter)?;
                let point = followed(jump.point, t.doc, t.buffer);
                (jump.file == t.file).then(|| t.buffer.offset_at_point(point))
            }
            name => t.doc.marks.get(t.buffer, name.of_buffer()?),
        }
    }

    /// `'x` va al primer no blanco de la línea de la mark y `` `x `` a su
    /// posición; con un operador pendiente son su motion y en Visual
    /// extienden la selección principal. Como en vim, ambos dejan el cursor
    /// de antes como la mark de `''`. Una mark `A`–`Z` de otro archivo se
    /// la pide a quien aloja el motor, solo en Normal. Una que no está
    /// puesta es `E20`.
    pub(super) fn go_to_mark(
        &mut self,
        c: char,
        exact: bool,
        t: &mut Target<'_>,
    ) -> Result<Option<KeyOutcome>, VimError> {
        let Some(name) = MarkName::parse(c) else {
            self.fail();
            return Ok(None);
        };
        if let Some(landing) = self.mark_elsewhere(name, exact, t.file) {
            return self.go_to_other_file(landing, t);
        }
        let subject = Subject::Mark(MarkJump { name, exact });
        let object = self.object_of(&subject, t);
        if object.is_none() {
            self.fail();
            return Ok(Some(KeyOutcome::InvalidCommand(CommandError::MarkNotSet)));
        }
        self.jumped(t)?;
        if let Mode::OperatorPending(operator) = self.mode {
            self.operate_on(operator, subject, object, t)?;
            return Ok(None);
        }
        self.reset();
        if let Some(Object::Mark { offset, exact }) = object {
            let head = mark::destination(t.buffer, offset, exact);
            let anchor = match self.mode {
                Mode::Visual(_) => t.selections.primary().anchor,
                _ => head,
            };
            *t.selections = Selections::single(Selection {
                anchor,
                head,
                goal: Goal::None,
            });
        }
        Ok(None)
    }

    /// La mark `A`–`Z`, si es de otro archivo.
    fn mark_elsewhere(&self, name: MarkName, exact: bool, file: FileId) -> Option<Landing> {
        let MarkName::File(letter) = name else {
            return None;
        };
        let jump = self.file_marks.get(letter)?;
        (jump.file != file).then_some(Landing {
            file: jump.file,
            point: jump.point,
            first_non_blank: !exact,
        })
    }

    /// Como vim, una mark de otro archivo no sirve de motion ni en Visual.
    fn go_to_other_file(
        &mut self,
        landing: Landing,
        t: &mut Target<'_>,
    ) -> Result<Option<KeyOutcome>, VimError> {
        if self.mode != Mode::Normal {
            self.fail();
            return Ok(None);
        }
        self.reset();
        self.jumped(t)?;
        Ok(Some(KeyOutcome::SwitchFile(landing)))
    }
}

/// `point` de una mark `A`–`Z` tras lo editado que todavía no siguió, como
/// un rango de `:g` a medio correr.
fn followed(point: Point, doc: &DocState, buffer: &Buffer) -> Point {
    doc.unfollowed(buffer)
        .iter()
        .fold(point, |point, splice| splice.lines.keep(point))
}
