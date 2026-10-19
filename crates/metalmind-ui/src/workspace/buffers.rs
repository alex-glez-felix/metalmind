//! Todos los documentos a la vez. Como vim con `hidden`, el default de nvim,
//! cambiar el documento de un panel ([`super::show`]) nunca pierde cambios:
//! la protección está al salir, y la ventana no se cierra mientras algún
//! documento, visible u oculto, tenga cambios.

use metalmind_vault::SaveKind;

use super::{DocumentId, Message, Workspace, WorkspaceError};
use crate::document::{Document, SaveError};

impl Workspace {
    /// Si se puede cerrar la ventana sin perder cambios; con autoguardado,
    /// antes guarda lo que pueda. Si no, como nvim, muestra el primer
    /// documento con cambios (el del panel enfocado, el de otro panel o uno
    /// oculto) y avisa cuál es.
    pub fn may_close_window(&mut self) -> bool {
        self.autosave_all();
        let Some(id) = self.first_unsaved() else {
            return true;
        };
        if let Err(error) = self.go_to(id) {
            self.report(&error);
            return false;
        }
        if let Some(document) = self.documents.get(id) {
            self.message = Some(Message::unsaved_in(document));
        }
        false
    }

    /// En el orden de vim: el del panel enfocado, los de los paneles y los
    /// ocultos, del más viejo al más nuevo.
    fn first_unsaved(&self) -> Option<DocumentId> {
        let focused = self.panes.get(&self.focused()).map(|pane| pane.document);
        let shown = self
            .tree
            .panes()
            .into_iter()
            .filter_map(|pane| self.panes.get(&pane).map(|pane| pane.document));
        let all = self.documents.iter().map(|(id, _)| id);
        focused
            .into_iter()
            .chain(shown)
            .chain(all)
            .find(|&id| self.documents.get(id).is_some_and(Document::is_modified))
    }

    /// Le da el foco a un panel que muestre `id` o, si está oculto, lo
    /// muestra en el panel enfocado; como en nvim, eso es un salto que
    /// `ctrl-o` deshace.
    fn go_to(&mut self, id: DocumentId) -> Result<(), WorkspaceError> {
        let focused = self.focused();
        let showing = self
            .tree
            .panes()
            .into_iter()
            .find(|pane| self.panes.get(pane).is_some_and(|p| p.document == id));
        if let Some(pane) = showing {
            return self.focus(pane);
        }
        self.push_jump(focused)?;
        self.show_document(focused, id)
    }

    /// `:wa`: guarda cada documento con cambios, también los ocultos, y
    /// devuelve si se guardaron todos. Como vim, uno que no se puede guardar
    /// no detiene a los demás; el aviso es el primer error o, si no hubo,
    /// lo último que se escribió.
    pub(super) fn save_all(&mut self) -> bool {
        let modified: Vec<DocumentId> = self
            .documents
            .iter()
            .filter(|(_, document)| document.is_modified())
            .map(|(id, _)| id)
            .collect();
        let mut first_error = None;
        for id in modified {
            let Some(document) = self.documents.get_mut(id) else {
                continue;
            };
            let message = match document.save(self.disk.as_ref(), SaveKind::Explicit) {
                Ok(stats) => {
                    let message = Message::written(&document.display_name(), stats);
                    self.saved(id);
                    message
                }
                Err(SaveError::NoFileName) => Message::no_file_name_for(id),
                Err(error) => {
                    log::error!("{error}");
                    Message::Error(error.to_string())
                }
            };
            if matches!(message, Message::Error(_)) {
                first_error.get_or_insert(message);
            } else {
                self.message = Some(message);
            }
        }
        match first_error {
            Some(error) => {
                self.message = Some(error);
                false
            }
            None => true,
        }
    }
}

#[cfg(test)]
#[path = "buffers_tests.rs"]
mod tests;
