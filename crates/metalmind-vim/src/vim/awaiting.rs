//! Lo que espera un carácter en la tecla siguiente: `f`, `"`, ….

use super::Vim;
use crate::Target;
use crate::register::RegisterName;
use crate::{Action, CharSearch, Key, KeyOutcome, VimError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Awaiting {
    /// `f`, `F`, `t` o `T`: el carácter a buscar.
    Find(CharSearch),
    /// `"`: el nombre del registro.
    Register,
    /// `q`: el registro donde grabar.
    Record,
    /// `@`: el registro que reproducir.
    Play,
    /// `m`: la mark que poner.
    SetMark,
    /// `'` o `` ` ``: la mark a la que ir.
    Mark { exact: bool },
    /// `r` en Visual: el carácter que reemplaza a los seleccionados.
    Replace,
}

impl Vim {
    /// Completa lo que esperaba un carácter; una tecla que no es carácter
    /// cancela, como `<Esc>`.
    pub(super) fn complete(
        &mut self,
        awaiting: Awaiting,
        key: Key,
        t: &mut Target<'_>,
    ) -> Result<KeyOutcome, VimError> {
        let Some(c) = awaited_char(key) else {
            self.reset();
            return Ok(KeyOutcome::Executed(Action::NormalMode));
        };
        let action = match awaiting {
            Awaiting::Find(search) => {
                self.find_char(search, c, t)?;
                Action::FindChar(search)
            }
            Awaiting::Register => {
                self.select_register(c);
                Action::SelectRegister
            }
            Awaiting::Record => {
                self.start_recording(c);
                Action::RecordMacro
            }
            Awaiting::Play => {
                if let Some(error) = self.play_macro(c, t.clipboard.as_deref_mut()) {
                    return Ok(KeyOutcome::InvalidCommand(error));
                }
                Action::PlayMacro
            }
            Awaiting::SetMark => {
                self.set_mark(c, t)?;
                Action::SetMark
            }
            Awaiting::Mark { exact } => {
                if let Some(outcome) = self.go_to_mark(c, exact, t)? {
                    return Ok(outcome);
                }
                Action::GoToMark { exact }
            }
            Awaiting::Replace => {
                self.replace_char(c, t)?;
                Action::Replace
            }
        };
        Ok(KeyOutcome::Executed(action))
    }

    /// `"x`: lo siguiente usa ese registro. El count de antes se multiplica
    /// con el de después.
    fn select_register(&mut self, c: char) {
        match RegisterName::parse(c) {
            Some(name) => {
                self.operator_count = self.take_count();
                self.selected = Some(name);
            }
            None => self.fail(),
        }
    }
}

fn awaited_char(key: Key) -> Option<char> {
    match key {
        Key::Char(c) => Some(c),
        Key::Tab => Some('\t'),
        _ => None,
    }
}
