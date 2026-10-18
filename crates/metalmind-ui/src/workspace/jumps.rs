//! Los saltos que cruzan archivos: los del motor que caen en otro archivo
//! (`ctrl-o`, `ctrl-i`, `'A`) y abrir otra nota, que también es un salto.
//! El vault avisa de renombres y borrados para que la jumplist y las marks
//! `A`–`Z` sigan al archivo.

use std::path::Path;

use metalmind_vault::VaultChange;
use metalmind_vim::{Clipboard, KeyOutcome, Landing};

use super::{Message, PaneId, Workspace, WorkspaceError, shown_path};

impl Workspace {
    /// Antes de que `pane` muestre otra cosa, su cursor queda en su
    /// jumplist y como la mark de `''`.
    pub(super) fn push_jump(&mut self, pane: PaneId) -> Result<(), WorkspaceError> {
        self.edit_pane(pane, |vim, buffer, selections| {
            Ok(vim.push_jump(buffer, selections)?)
        })
    }

    /// Lo que devolvió el motor tras una tecla: si es un salto a otro
    /// archivo, se hace aquí y la macro que lo tecleó sigue. Devuelve el
    /// próximo comando de la macro, si llega a uno.
    pub(super) fn settle(
        &mut self,
        outcome: KeyOutcome,
        clipboard: &mut dyn Clipboard,
    ) -> Result<KeyOutcome, WorkspaceError> {
        let KeyOutcome::SwitchFile(landing) = outcome else {
            return Ok(outcome);
        };
        if !self.land(self.focused(), landing)? {
            self.vim.abort_macro();
            return Ok(outcome);
        }
        Ok(self
            .resume_macro(clipboard)
            .map_or(outcome, KeyOutcome::Command))
    }

    /// `pane` pasa al archivo de `landing` y el motor pone el cursor. Si
    /// el archivo ya no existe, no se mueve: el motor lo salta desde ahora.
    pub(super) fn land(&mut self, pane: PaneId, landing: Landing) -> Result<bool, WorkspaceError> {
        let shown = match self.documents.of_file(landing.file) {
            Some(id) => {
                self.show_document(pane, id)?;
                true
            }
            None => match self.files.path(landing.file).map(Path::to_owned) {
                Some(path) if path.exists() => self.show_file(pane, &shown_path(&path))?.is_some(),
                gone => {
                    self.vim.file_removed(landing.file);
                    let name = gone.map_or_else(String::new, |path| {
                        format!(": {}", shown_path(&path).display())
                    });
                    self.message = Some(Message::Error(format!("el archivo ya no existe{name}")));
                    false
                }
            },
        };
        if shown {
            self.edit_pane(pane, |vim, buffer, selections| {
                vim.land(landing, buffer, selections);
                Ok(())
            })?;
        }
        Ok(shown)
    }

    /// Un lote del watcher del vault de raíz `root`: los archivos movidos
    /// desde fuera se llevan sus documentos abiertos, su lugar en la
    /// jumplist y sus marks `A`–`Z`; los borrados pierden sus marks y
    /// `ctrl-o` salta sus entradas.
    pub fn files_changed(&mut self, root: &Path, changes: &[VaultChange]) {
        for change in changes {
            match change {
                VaultChange::Renamed { from, to, .. } => {
                    self.moved(&root.join(from.as_str()), &root.join(to.as_str()));
                }
                VaultChange::Removed { path, .. } => {
                    for file in self.files.within(&root.join(path.as_str())) {
                        self.vim.file_removed(file);
                    }
                }
                VaultChange::Created { path, .. } => {
                    for file in self.files.within(&root.join(path.as_str())) {
                        self.vim.file_restored(file);
                    }
                }
                VaultChange::Modified { .. } => {}
            }
        }
    }
}

#[cfg(test)]
#[path = "jumps_tests.rs"]
mod tests;
