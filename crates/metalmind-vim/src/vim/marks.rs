//! `m{a-z}`, `'{a-z}` y `` `{a-z} ``, y `''` y ` `` ` para volver.

use metalmind_buffer::{Buffer, Goal, Selection, Selections};

use super::Vim;
use crate::dot::{MarkJump, Subject};
use crate::mark::{self, MarkName};
use crate::object::Object;
use crate::register::Clipboard;
use crate::{Mode, VimError};

impl Vim {
    /// `m{a-z}` en el cursor principal; `m'` pone la del salto anterior.
    pub(super) fn set_mark(
        &mut self,
        c: char,
        buffer: &mut Buffer,
        selections: &Selections,
    ) -> Result<(), VimError> {
        self.reset();
        match MarkName::parse(c) {
            Some(name) => self.marks.set(buffer, name, selections.primary().head)?,
            None => self.fail(),
        }
        Ok(())
    }

    /// `'x` va al primer no blanco de la línea de la mark y `` `x `` a su
    /// posición; con un operador pendiente son su motion y en Visual
    /// extienden la selección principal. Como en vim, ambos dejan el cursor
    /// de antes como la mark de `''`.
    pub(super) fn go_to_mark(
        &mut self,
        c: char,
        exact: bool,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<(), VimError> {
        let Some(name) = MarkName::parse(c) else {
            self.fail();
            return Ok(());
        };
        let subject = Subject::Mark(MarkJump { name, exact });
        let object = self.object_of(&subject, buffer);
        if object.is_some() {
            self.jumped(buffer, selections)?;
        }
        if let Mode::OperatorPending(operator) = self.mode {
            return self.operate_on(operator, subject, object, buffer, selections, clipboard);
        }
        self.reset();
        match object {
            Some(Object::Mark { offset, exact }) => {
                let head = mark::destination(buffer, offset, exact);
                let anchor = match self.mode {
                    Mode::Visual(_) => selections.primary().anchor,
                    _ => head,
                };
                *selections = Selections::single(Selection {
                    anchor,
                    head,
                    goal: Goal::None,
                });
            }
            _ => self.fail(),
        }
        Ok(())
    }

    /// Antes de un salto: la mark de `''` queda donde está el cursor.
    pub(super) fn jumped(
        &mut self,
        buffer: &mut Buffer,
        selections: &Selections,
    ) -> Result<(), VimError> {
        let head = selections.primary().head;
        Ok(self.marks.set(buffer, MarkName::Previous, head)?)
    }
}
