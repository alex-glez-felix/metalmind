//! La palabra que se escribe en el argumento de un comando de `:` y cómo
//! aceptar su autocompletado (#156).

use metalmind_vim::TypedArgument;

use super::{PaneId, Workspace};

impl Workspace {
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
