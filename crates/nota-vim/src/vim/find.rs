//! `f`, `F`, `t`, `T`, `;` y `,`.

use nota_buffer::{Buffer, Selections};

use super::{Vim, move_cursors};
use crate::dot::Subject;
use crate::find::Find;
use crate::object::Object;
use crate::{Action, CharSearch, Key, KeyOutcome, Mode, VimError};

impl Vim {
    /// Completa `f`, `F`, `t` o `T`; una tecla que no es carácter se usa
    /// para cancelar, como `<Esc>`.
    pub(super) fn find_char(
        &mut self,
        search: CharSearch,
        key: Key,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<KeyOutcome, VimError> {
        let Some(target) = searched_char(key) else {
            self.reset();
            return Ok(KeyOutcome::Executed(Action::NormalMode));
        };
        let find = Find::new(search, target);
        self.last_find = Some(find);
        self.jump(find, Object::Find(find).into(), buffer, selections)?;
        Ok(KeyOutcome::Executed(Action::FindChar(search)))
    }

    /// `;` o `,`; sin una búsqueda anterior solo cancela lo pendiente.
    pub(super) fn repeat_find(
        &mut self,
        reversed: bool,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        let Some(find) = self.last_find else {
            self.reset();
            return Ok(());
        };
        let subject = Subject::RepeatFind { reversed };
        self.jump(find.repeat(reversed), subject, buffer, selections)
    }

    /// Con un operador pendiente opera sobre `subject`, que `.` repite.
    fn jump(
        &mut self,
        find: Find,
        subject: Subject,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        if let Mode::OperatorPending(operator) = self.mode {
            return self.operate(operator, subject, buffer, selections);
        }
        let times = self.count.take().unwrap_or(1);
        *selections = move_cursors(selections, |s| find.resolve(buffer, s.head, times));
        Ok(())
    }
}

fn searched_char(key: Key) -> Option<char> {
    match key {
        Key::Char(c) => Some(c),
        Key::Tab => Some('\t'),
        _ => None,
    }
}
