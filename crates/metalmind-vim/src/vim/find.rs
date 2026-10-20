//! `f`, `F`, `t`, `T`, `;` y `,`.

use super::Vim;
use super::dispatch::move_cursors;
use crate::Target;
use crate::dot::Subject;
use crate::find::Find;
use crate::object::Object;
use crate::{CharSearch, Mode, VimError};

impl Vim {
    /// Completa `f`, `F`, `t` o `T` con el carácter a buscar.
    pub(super) fn find_char(
        &mut self,
        search: CharSearch,
        target: char,
        t: &mut Target<'_>,
    ) -> Result<(), VimError> {
        let find = Find::new(search, target);
        self.last_find = Some(find);
        self.jump(find, Object::Find(find).into(), t)
    }

    /// `;` o `,`; sin una búsqueda anterior solo cancela lo pendiente.
    pub(super) fn repeat_find(
        &mut self,
        reversed: bool,
        t: &mut Target<'_>,
    ) -> Result<(), VimError> {
        let Some(find) = self.last_find else {
            self.fail();
            return Ok(());
        };
        let subject = Subject::RepeatFind { reversed };
        self.jump(find.repeat(reversed), subject, t)
    }

    /// Con un operador pendiente opera sobre `subject`, que `.` repite.
    fn jump(&mut self, find: Find, subject: Subject, t: &mut Target<'_>) -> Result<(), VimError> {
        if let Mode::OperatorPending(operator) = self.mode {
            return self.operate(operator, subject, t);
        }
        let times = self.take_count().unwrap_or(1);
        self.reset();
        let visual = self.in_visual();
        match move_cursors(t.selections, visual, |s| {
            find.resolve(t.buffer, s.head, times)
        }) {
            Some(moved) => *t.selections = moved,
            None => self.fail(),
        }
        Ok(())
    }
}
