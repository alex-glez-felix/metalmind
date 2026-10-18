//! `.`: repetir el último cambio.

use nota_buffer::{Buffer, Selections};

use super::Vim;
use crate::dot::{Change, Subject};
use crate::object::Object;
use crate::register::{Clipboard, RegisterName};
use crate::{Mode, VimError};

impl Vim {
    /// Un count reemplaza el del cambio y queda para los `.` siguientes. Si
    /// falla, `.` sigue repitiendo el cambio de antes, con su count. Usa el
    /// registro del cambio, salvo `"1`–`"8`, que avanzan al siguiente.
    pub(super) fn repeat_change(
        &mut self,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<(), VimError> {
        let count = self.count.take();
        self.reset();
        // Se clona porque repetirlo lo vuelve a grabar, ya con el count nuevo.
        let Some(dot) = self.last_change.clone() else {
            self.fail();
            return Ok(());
        };
        self.count = count.or(dot.count);
        self.selected = dot.register.map(RegisterName::next_for_repeat);
        // Como vim, repetir `d/foo<CR>` o `d*` vuelve a teclear la búsqueda:
        // `n` sigue buscando lo que buscó.
        match &dot.change {
            Change::Operate(_, Subject::Object(Object::Search(query))) => {
                self.search.remember(query);
            }
            Change::Operate(_, Subject::Object(Object::SearchWord(search))) => {
                self.remember_word(*search, buffer, selections);
            }
            _ => {}
        }
        match dot.change {
            Change::Operate(operator, subject) => {
                self.operate(operator, subject, buffer, selections, clipboard)?;
            }
            Change::Paste(side) => self.paste(side, buffer, selections, clipboard)?,
            Change::Insert(at) => self.enter_insert(at, buffer, selections)?,
            Change::Replace(c, size) => self.replace_sized(c, size, buffer, selections)?,
        }
        if self.mode != Mode::Insert {
            return Ok(());
        }
        for typed in &dot.typed {
            self.type_in_insert(typed, buffer, selections)?;
        }
        self.repeat_insert(buffer, selections)?;
        self.leave_insert(buffer, selections)
    }
}
