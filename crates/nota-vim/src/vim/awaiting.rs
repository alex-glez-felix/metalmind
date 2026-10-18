//! Lo que espera un carácter en la tecla siguiente: `f`, `"`, ….

use nota_buffer::{Buffer, Selections};

use super::Vim;
use crate::register::{Clipboard, RegisterName};
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
}

impl Vim {
    /// Completa lo que esperaba un carácter; una tecla que no es carácter
    /// cancela, como `<Esc>`.
    pub(super) fn complete(
        &mut self,
        awaiting: Awaiting,
        key: Key,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<KeyOutcome, VimError> {
        let Some(c) = awaited_char(key) else {
            self.reset();
            return Ok(KeyOutcome::Executed(Action::NormalMode));
        };
        let action = match awaiting {
            Awaiting::Find(search) => {
                self.find_char(search, c, buffer, selections, clipboard)?;
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
                self.play_macro(c, clipboard);
                Action::PlayMacro
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
