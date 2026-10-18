//! Cambiar el documento de un panel: la única función que lo hace, para
//! `:e`, el árbol, el finder, seguir un link y los saltos de la jumplist.

use std::path::Path;

use super::{DocumentId, Pane, PaneId, Workspace, WorkspaceError};
use crate::document::Document;
use crate::workspace::Message;

impl Workspace {
    /// `pane` pasa a mostrar `path`, abriéndolo si no está abierto. Como
    /// vim con `hidden`, no se niega por cambios sin guardar: el documento
    /// de antes queda oculto, con sus cambios. Lo que estuviera a medias en
    /// el panel termina como con `ctrl-c`. Devuelve si lo muestra; si no se
    /// pudo leer, el error queda en [`Workspace::message`].
    ///
    /// # Errors
    /// Solo si el estado está inconsistente, lo que sería un bug.
    pub fn show_file(&mut self, pane: PaneId, path: &Path) -> Result<bool, WorkspaceError> {
        let id = match self.documents.find(path) {
            Some(id) => id,
            None => match Document::open(path) {
                Ok(document) => self.insert_document(document),
                Err(error) => {
                    self.interrupt(pane)?;
                    self.message = Some(Message::Error(error.to_string()));
                    return Ok(false);
                }
            },
        };
        self.show_document(pane, id)?;
        Ok(true)
    }

    /// `pane` pasa a mostrar `id`, con el cursor de un panel que ya lo
    /// muestra; sus números y su jumplist se quedan.
    pub(super) fn show_document(
        &mut self,
        pane: PaneId,
        id: DocumentId,
    ) -> Result<(), WorkspaceError> {
        self.interrupt(pane)?;
        let current = self.pane(pane)?;
        if current.document == id {
            return Ok(());
        }
        let (numbers, jumps) = (current.numbers, current.jumps.clone());
        let shown = Pane {
            jumps,
            ..self.pane_on(id, numbers)
        };
        self.message = Some(Message::opened(self.document(id)?));
        self.vim.cursor_moved();
        self.panes.insert(pane, shown);
        self.drop_unshown();
        Ok(())
    }
}
