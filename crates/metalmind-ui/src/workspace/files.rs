//! Lo que el panel lateral le pide a los paneles: abrir un archivo del
//! vault y renombrarlo sin que los buffers abiertos pierdan su archivo.

use std::path::{Path, PathBuf};

use metalmind_vault::{FileOpError, Vault, VaultPath};

use metalmind_buffer::{Buffer, ByteOffset, Selection, Selections};

use super::pane_tree::Axis;
use super::{DocumentId, Pane, PaneId, Workspace, WorkspaceError, clamp_cursor};
use crate::document::{Document, entry_path};

/// Una línea y una columna en bytes dentro de ella, las dos desde 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinePosition {
    pub line: usize,
    pub column: usize,
}

impl LinePosition {
    /// El offset en `buffer`, dentro del texto y en frontera de carácter: el
    /// archivo pudo cambiar desde que se buscó.
    fn offset_in(self, buffer: &Buffer) -> ByteOffset {
        let start = buffer.line_start(self.line);
        let mut offset = (start + self.column).min(buffer.line_end(buffer.line_of(start)));
        while !buffer.is_char_boundary(offset) {
            offset = offset.saturating_sub(1);
        }
        offset
    }
}

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
    /// enfocado lo muestra. En el panel enfocado es como `:e`: el documento
    /// de antes queda oculto. Un archivo que no se puede leer no cambia
    /// nada y el aviso queda en [`Workspace::message`].
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

    /// Como [`Workspace::open_file`], con el cursor en `at` si el panel
    /// enfocado muestra `path`: lo que abre un resultado de `space /`. Es un
    /// salto: abrir otra nota ya lo deja en la jumplist, y moverse dentro de
    /// la que se ve también, como `:123`.
    ///
    /// # Errors
    /// Solo si el estado está inconsistente, lo que sería un bug.
    pub fn open_file_at(
        &mut self,
        path: &Path,
        placement: Placement,
        at: LinePosition,
    ) -> Result<bool, WorkspaceError> {
        let shown = placement == Placement::Current && self.focused_document()?.is_file(path);
        if !self.open_file(path, placement)? {
            return Ok(false);
        }
        if shown {
            // Como al cambiar de nota: lo que estuviera a medias termina.
            self.interrupt(self.focused())?;
            self.push_jump(self.focused())?;
        }
        self.edit_pane(self.focused(), |vim, buffer, selections| {
            vim.cursor_moved();
            let head = clamp_cursor(buffer, at.offset_in(buffer), vim.mode());
            *selections = Selections::single(Selection::cursor(head));
            Ok(())
        })?;
        Ok(true)
    }

    /// Los documentos con cambios sin guardar que tienen archivo, también
    /// los que todavía no existen en disco: su texto no es el del disco.
    pub fn unsaved_files(&self) -> impl Iterator<Item = (&Path, &Document)> {
        self.documents
            .iter()
            .filter(|(_, document)| document.is_modified())
            .filter_map(|(_, document)| Some((document.path()?, document)))
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
        vault.rename(from, to)?;
        let root = vault.root();
        self.moved(&root.join(from.as_str()), &root.join(to.as_str()));
        Ok(())
    }

    /// `old` (un archivo o una carpeta) pasó a llamarse `new`: sus
    /// documentos abiertos, con sus cambios, su lugar en la jumplist y sus
    /// marks `A`–`Z` lo siguen.
    pub(super) fn moved(&mut self, old: &Path, new: &Path) {
        for (id, rest) in self.documents_within(old) {
            if let Some(document) = self.documents.get_mut(id) {
                let path = if rest.as_os_str().is_empty() {
                    new.to_owned()
                } else {
                    new.join(rest)
                };
                document.set_path(shown_path(&path));
            }
        }
        self.files.rename(old, new);
        for file in self.files.within(new) {
            self.vim.file_restored(file);
        }
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
        for file in self.files.within(&vault.root().join(path.as_str())) {
            self.vim.file_removed(file);
        }
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
            } else if let Some(numbers) = self.panes.get(&pane).map(|state| state.numbers) {
                let id = self.insert_document(Document::unnamed());
                let jumps = self.panes.get(&pane).map(|state| state.jumps.clone());
                let mut state = Pane::at_start(id, numbers, self.tabstop);
                state.jumps = jumps.unwrap_or_default();
                self.panes.insert(pane, state);
            }
        }
        for id in gone {
            self.forget(id);
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
/// trabaja Metalmind) si está dentro.
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
