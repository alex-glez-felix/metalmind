//! `f`, `F`, `t`, `T`, `;` y `,`.

use nota_buffer::{Buffer, Selections};

use super::Vim;
use super::dispatch::move_cursors;
use crate::dot::Subject;
use crate::find::Find;
use crate::object::Object;
use crate::register::Clipboard;
use crate::{CharSearch, Mode, VimError};

impl Vim {
    /// Completa `f`, `F`, `t` o `T` con el carácter a buscar.
    pub(super) fn find_char(
        &mut self,
        search: CharSearch,
        target: char,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<(), VimError> {
        let find = Find::new(search, target);
        self.last_find = Some(find);
        self.jump(
            find,
            Object::Find(find).into(),
            buffer,
            selections,
            clipboard,
        )
    }

    /// `;` o `,`; sin una búsqueda anterior solo cancela lo pendiente.
    pub(super) fn repeat_find(
        &mut self,
        reversed: bool,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<(), VimError> {
        let Some(find) = self.last_find else {
            self.fail();
            return Ok(());
        };
        let subject = Subject::RepeatFind { reversed };
        self.jump(
            find.repeat(reversed),
            subject,
            buffer,
            selections,
            clipboard,
        )
    }

    /// Con un operador pendiente opera sobre `subject`, que `.` repite.
    fn jump(
        &mut self,
        find: Find,
        subject: Subject,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<(), VimError> {
        if let Mode::OperatorPending(operator) = self.mode {
            return self.operate(operator, subject, buffer, selections, clipboard);
        }
        let times = self.take_count().unwrap_or(1);
        self.reset();
        let visual = self.in_visual();
        match move_cursors(selections, visual, |s| find.resolve(buffer, s.head, times)) {
            Some(moved) => *selections = moved,
            None => self.fail(),
        }
        Ok(())
    }
}
