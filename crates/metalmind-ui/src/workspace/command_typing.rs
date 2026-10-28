//! Lo que se escribe tras `:` en el panel con el foco: el nombre de un
//! comando o la palabra de su argumento (#156), y cómo aceptar su
//! autocompletado.

use metalmind_vim::{TypedArgument, TypedCommand};

use super::{PaneId, Workspace};

impl Workspace {
    /// Lo que se escribe tras `:` en `pane`, si tiene el foco, mientras sea
    /// nada o el nombre de un comando.
    #[must_use]
    pub fn typed_command(&self, pane: PaneId) -> Option<TypedCommand> {
        (pane == self.focused())
            .then(|| self.vim.typed_command())
            .flatten()
    }

    /// Cambia lo de [`Workspace::typed_command`] por `text`. Si `pane` no
    /// escribe un nombre de comando, no hace nada.
    pub fn complete_command(&mut self, pane: PaneId, text: &str) {
        if self.typed_command(pane).is_some() {
            self.vim.complete_command(text);
        }
    }

    /// Lo que se escribe tras `:comando ` en `pane`, si tiene el foco.
    #[must_use]
    pub fn typed_argument(&self, pane: PaneId) -> Option<TypedArgument> {
        (pane == self.focused())
            .then(|| self.vim.typed_argument())
            .flatten()
    }

    /// Cambia la palabra de [`Workspace::typed_argument`] por `text`, ya
    /// escrito con sus escapes. Si `pane` no escribe un argumento, no hace
    /// nada.
    pub fn complete_argument(&mut self, pane: PaneId, text: &str) {
        if self.typed_argument(pane).is_some() {
            self.vim.complete_argument(text);
        }
    }
}
