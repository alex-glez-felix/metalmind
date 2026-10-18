//! Lo que el panel lateral le pide a los paneles: abrir un archivo del
//! vault y renombrarlo sin que los buffers abiertos pierdan su archivo.

use std::path::{Path, PathBuf};

use nota_vault::{FileOpError, Vault, VaultPath};

use super::pane_tree::Axis;
use super::{DocumentId, Pane, PaneId, Workspace, WorkspaceError};
use crate::document::{Document, entry_path};

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
            Placement::Split(axis) => self.open_in_split(axis, path)?,
        }
        Ok(self.focused_document()?.is_file(path))
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
                document.set_path(shown_path(&path));
            }
        }
        Ok(())
    }

    /// Manda `path` a la papelera de `vault` y cierra los documentos de lo
    /// que se borró, con sus cambios: los paneles que los mostraban se
    /// cierran y, si era el último, queda en `[No Name]`. Así un `:w` no
    /// vuelve a crear lo que se acaba de borrar.
    ///
    /// # Errors
    /// Los de [`Vault::move_to_trash`]; los documentos quedan como estaban.
    pub fn trash_file(&mut self, vault: &Vault, path: &VaultPath) -> Result<(), FileOpError> {
        let gone: Vec<DocumentId> = self
            .documents_within(&vault.root().join(path.as_str()))
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        vault.move_to_trash(path)?;
        let showing: Vec<PaneId> = self
            .panes
            .iter()
            .filter(|(_, pane)| gone.contains(&pane.document))
            .map(|(&id, _)| id)
            .collect();
        if showing.contains(&self.focused())
            && let Err(error) = self.interrupt(self.focused())
        {
            self.report(&error);
        }
        for pane in showing {
            if self.tree.len() > 1 {
                self.close(pane);
            } else if let Some(state) = self.panes.get_mut(&pane) {
                let id = self.documents.insert(Document::unnamed());
                *state = Pane::at_start(id, state.numbers);
                self.drop_unshown();
            }
        }
        self.vim.cursor_moved();
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
        let root = entry_path(path);
        self.documents
            .iter()
            .filter_map(|(id, document)| {
                let own = document.entry_path()?;
                let rest = own.strip_prefix(&root).ok()?.to_owned();
                Some((id, rest))
            })
            .collect()
    }
}

/// La ruta como la escribiría quien usa vim, y como la muestran el título
/// y la barra: relativa al directorio de trabajo (la raíz del vault, donde
/// trabaja nota) si está dentro.
#[must_use]
pub fn shown_path(path: &Path) -> PathBuf {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| path.strip_prefix(cwd).ok().map(Path::to_owned))
        .filter(|relative| !relative.as_os_str().is_empty())
        .unwrap_or_else(|| path.to_owned())
}

#[cfg(test)]
#[path = "files_tests.rs"]
mod tests;
