//! Lo que el panel lateral le pide a los paneles: abrir un archivo del
//! vault y renombrarlo sin que los buffers abiertos pierdan su archivo.

use std::path::{Path, PathBuf};

use nota_vault::{FileOpError, Vault, VaultPath};

use super::pane_tree::Axis;
use super::{DocumentId, Message, Workspace, WorkspaceError};
use crate::document::Document;

/// Dónde se abre un archivo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    /// En el panel enfocado, como `:e`.
    Current,
    /// En un split nuevo del panel enfocado, como `ctrl-w v` o `ctrl-w s`
    /// seguido de `:e`.
    Split(Axis),
}

impl Workspace {
    /// Muestra `path` donde dice `placement` y devuelve si el panel
    /// enfocado lo muestra. En el panel enfocado se niega, como `:e`, si
    /// se perderían cambios; un archivo que no se puede leer no abre
    /// ningún split. En los dos casos, el aviso queda en
    /// [`Workspace::message`].
    ///
    /// # Errors
    /// Solo si el estado está inconsistente, lo que sería un bug.
    pub fn open_file(&mut self, path: &Path, placement: Placement) -> Result<bool, WorkspaceError> {
        match placement {
            Placement::Current => {
                if !self.focused_document()?.is_file(path) {
                    self.edit(Some(path), false)?;
                }
            }
            Placement::Split(axis) => {
                let Some(id) = self.find_or_open(path) else {
                    return Ok(false);
                };
                let numbers = self.pane(self.focused())?.numbers;
                self.split(axis)?;
                let shown = self.pane_on(id, numbers);
                self.panes.insert(self.focused(), shown);
                self.message = Some(Message::opened(self.document(id)?));
                self.vim.cursor_moved();
            }
        }
        Ok(self.focused_document()?.is_file(path))
    }

    fn find_or_open(&mut self, path: &Path) -> Option<DocumentId> {
        if let Some(id) = self.documents.find(path) {
            return Some(id);
        }
        match Document::open(path) {
            Ok(document) => Some(self.documents.insert(document)),
            Err(error) => {
                self.message = Some(Message::Error(error.to_string()));
                None
            }
        }
    }

    /// Renombra o mueve `from` a `to` en `vault`; los documentos abiertos de
    /// `from` (o de lo que contiene, si es carpeta) pasan a su ruta nueva y
    /// conservan sus cambios sin guardar.
    ///
    /// # Errors
    /// Los de [`Vault::rename`]; los documentos quedan como estaban.
    pub fn rename_file(
        &mut self,
        vault: &Vault,
        from: &VaultPath,
        to: &VaultPath,
    ) -> Result<(), FileOpError> {
        let old = vault.root().join(from.as_str());
        let moved = self.documents_within(&old);
        vault.rename(from, to)?;
        let new = vault.root().join(to.as_str());
        for (id, rest) in moved {
            if let Some(document) = self.documents.get_mut(id) {
                let path = if rest.as_os_str().is_empty() {
                    new.clone()
                } else {
                    new.join(rest)
                };
                document.set_path(path);
            }
        }
        Ok(())
    }

    /// Algún documento de `path` o de lo que contiene tiene cambios sin
    /// guardar.
    #[must_use]
    pub fn has_unsaved_within(&self, path: &Path) -> bool {
        self.documents_within(path)
            .into_iter()
            .filter_map(|(id, _)| self.documents.get(id))
            .any(Document::is_modified)
    }

    /// Los documentos de `path` o de lo que contiene, con su ruta relativa
    /// a `path` (vacía si es el mismo archivo).
    fn documents_within(&self, path: &Path) -> Vec<(DocumentId, PathBuf)> {
        let Ok(root) = path.canonicalize() else {
            return Vec::new();
        };
        self.documents
            .iter()
            .filter_map(|(id, document)| {
                let own = document.resolved_path()?;
                let rest = own.strip_prefix(&root).ok()?.to_owned();
                Some((id, rest))
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "files_tests.rs"]
mod tests;
