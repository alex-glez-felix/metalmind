//! `.`: repetir el último cambio.

use super::Vim;
use crate::Target;
use crate::dot::{Change, Subject};
use crate::object::Object;
use crate::register::RegisterName;
use crate::visual::Area;
use crate::{Mode, VimError};

impl Vim {
    /// Un count reemplaza el del cambio y queda para los `.` siguientes. Si
    /// falla, `.` sigue repitiendo el cambio de antes, con su count. Usa el
    /// registro del cambio, salvo `"1`–`"8`, que avanzan al siguiente.
    pub(super) fn repeat_change(&mut self, t: &mut Target<'_>) -> Result<(), VimError> {
        let count = self.count.take();
        self.reset();
        // Se clona porque repetirlo lo vuelve a grabar, ya con el count nuevo.
        let Some(dot) = self.last_change.clone() else {
            self.fail();
            return Ok(());
        };
        // Como el `redo_VIsual` de vim, lo hecho en Visual se repite con su
        // count.
        self.count = match dot.change {
            Change::Block(..) | Change::Operate(_, Subject::Object(Object::Visual(_))) => dot.count,
            _ => count.or(dot.count),
        };
        self.selected = dot.register.map(RegisterName::next_for_repeat);
        // Como vim, repetir `d/foo<CR>` o `d*` vuelve a teclear la búsqueda:
        // `n` sigue buscando lo que buscó.
        match &dot.change {
            Change::Operate(_, Subject::Object(Object::Search(query))) => {
                self.search.remember(query);
            }
            Change::Operate(_, Subject::Object(Object::SearchWord(search))) => {
                self.remember_word(*search, t.buffer, t.selections);
            }
            _ => {}
        }
        match dot.change {
            Change::Operate(operator, Subject::Object(Object::Visual(Area::Sized(size)))) => {
                self.redo_visual(operator, size, t)?;
            }
            Change::Operate(operator, subject) => {
                self.operate(operator, subject, t)?;
            }
            Change::Paste(side) => self.paste(side, t)?,
            Change::Insert { at, list } => self.enter_insert(at, list, t)?,
            Change::Replace(c, size) => self.replace_sized(c, size, t)?,
            Change::Block(change, size) => {
                self.repeat_block(change, size, t)?;
            }
            Change::ToggleCheckbox => self.toggle_checkbox(t)?,
        }
        if self.mode != Mode::Insert {
            return Ok(());
        }
        for typed in &dot.typed {
            self.type_in_insert(typed, t)?;
        }
        self.finish_insert(t)
    }
}
