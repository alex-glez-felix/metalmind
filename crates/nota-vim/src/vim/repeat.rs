//! `.`: repetir el último cambio.

use nota_buffer::{Buffer, Selections};

use super::Vim;
use crate::dot::Change;
use crate::{Mode, VimError};

impl Vim {
    /// Un count reemplaza el del cambio y queda para los `.` siguientes. Si
    /// falla, `.` sigue repitiendo el cambio de antes, con su count.
    pub(super) fn repeat_change(
        &mut self,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        let count = self.count.take();
        self.reset();
        // Se clona porque repetirlo lo vuelve a grabar, ya con el count nuevo.
        let Some(dot) = self.last_change.clone() else {
            return Ok(());
        };
        self.count = count.or(dot.count);
        match dot.change {
            Change::Operate(operator, subject) => {
                self.operate(operator, subject, buffer, selections)?;
            }
            Change::Paste(side) => self.paste(side, buffer, selections)?,
            Change::Insert(at) => self.enter_insert(at, buffer, selections)?,
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
