//! Lo de un cambio del vault que va a los buffers con cambios sin guardar
//! en vez de al disco (05 §5): se planea sobre el texto del buffer, entra
//! en su undo y `:vaultundo` lo devuelve por el mismo camino.

use std::collections::HashSet;

use gpui_kit::Context;
use metalmind_vault::{Changes, FileEdits, Vault, VaultPath};

use super::VaultChanges;
use crate::tag_change::VaultConfigText;
use crate::workspace::FileEditError;

/// Un cambio partido: lo que se escribe en disco y las ediciones de los
/// archivos con cambios sin guardar, que van a su buffer.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Routed {
    pub disk: Changes,
    pub buffers: Vec<FileEdits>,
}

/// Parte `changes`: las ediciones de lo que `unsaved` dice que tiene
/// cambios sin guardar van a su buffer; lo demás, al disco.
pub(super) fn route(changes: &Changes, unsaved: impl Fn(&VaultPath) -> bool) -> Routed {
    let (buffers, edits) = changes
        .edits
        .iter()
        .cloned()
        .partition(|file| unsaved(&file.path));
    Routed {
        disk: Changes {
            edits,
            creates: changes.creates.clone(),
            renames: changes.renames.clone(),
        },
        buffers,
    }
}

/// Los buffers con cambios sin guardar del vault, con su texto como se
/// guardaría.
pub(super) struct UnsavedTexts {
    pub notes: Vec<(VaultPath, String)>,
    pub config: Option<String>,
}

impl UnsavedTexts {
    /// La config del vault como la ve quien la edita: la de su buffer, si
    /// tiene cambios, o la del disco.
    pub fn config(&self, root: &std::path::Path) -> VaultConfigText {
        self.config
            .clone()
            .map_or_else(|| VaultConfigText::read(root), VaultConfigText::Text)
    }
}

impl VaultChanges {
    pub(super) fn unsaved_texts(&self, cx: &Context<Self>) -> UnsavedTexts {
        let session = self.session.read(cx);
        let root = session.root();
        let mut texts = UnsavedTexts {
            notes: Vec::new(),
            config: None,
        };
        for (file, document) in self.workspace.read(cx).unsaved_files() {
            if let Some(path) = session.vault_path(file) {
                texts.notes.push((path, document.file_text()));
            } else if crate::tag_change::config_path(root, file).is_some() {
                texts.config = Some(document.file_text());
            }
        }
        texts
    }

    /// Si cada edición de `buffers` cabe en su buffer, sin aplicarla.
    ///
    /// # Errors
    /// El porqué de la primera que no.
    pub(super) fn check_buffers(
        &self,
        buffers: &[FileEdits],
        cx: &Context<Self>,
    ) -> Result<(), String> {
        let root = self.session.read(cx).root();
        let workspace = self.workspace.read(cx);
        buffers.iter().try_for_each(|file| {
            workspace
                .check_file_edits(&root.join(file.path.as_str()), file)
                .map_err(|error| error.to_string())
        })
    }

    /// El primero de `buffers` cuyo documento tiene un Insert abierto: no
    /// se le puede meter un paso de undo sin mezclarlo con lo tecleado.
    pub(super) fn typing_in_buffers(
        &self,
        buffers: &[FileEdits],
        cx: &Context<Self>,
    ) -> Option<VaultPath> {
        let root = self.session.read(cx).root();
        let workspace = self.workspace.read(cx);
        buffers
            .iter()
            .find(|file| {
                matches!(
                    workspace.check_file_edits(&root.join(file.path.as_str()), file),
                    Err(FileEditError::Typing(_))
                )
            })
            .map(|file| file.path.clone())
    }

    /// Aplica `buffers` a sus buffers, cada uno como un paso de su undo.
    /// Devuelve lo que los deshace y lo que no se pudo aplicar.
    pub(super) fn edit_buffers(
        &mut self,
        buffers: &[FileEdits],
        cx: &mut Context<Self>,
    ) -> (Vec<FileEdits>, Vec<(VaultPath, String)>) {
        let root = self.session.read(cx).root().to_owned();
        self.workspace.update(cx, |workspace, cx| {
            cx.notify();
            let mut undo = Vec::with_capacity(buffers.len());
            let mut failed = Vec::new();
            for file in buffers {
                match workspace.edit_file(&root.join(file.path.as_str()), file) {
                    Ok(back) => undo.push(back),
                    Err(error) => failed.push((file.path.clone(), error.to_string())),
                }
            }
            (undo, failed)
        })
    }

    /// Devuelve lo que un cambio hizo en buffers: al buffer si sigue con
    /// cambios sin guardar, o al disco si ya se guardó o se cerró. Devuelve
    /// los archivos que no se pudieron devolver.
    pub(super) fn undo_buffers(
        &mut self,
        vault: &Vault,
        buffers: &[FileEdits],
        cx: &mut Context<Self>,
    ) -> Vec<VaultPath> {
        let unsaved: HashSet<VaultPath> = self.unsaved_paths(cx);
        let Routed { disk, buffers } = route(
            &Changes {
                edits: buffers.to_vec(),
                ..Changes::default()
            },
            |path| unsaved.contains(path),
        );
        let (_, failed) = self.edit_buffers(&buffers, cx);
        let mut left: Vec<VaultPath> = failed
            .into_iter()
            .map(|(path, error)| {
                log::warn!("no se devolvió {path}: {error}");
                path
            })
            .collect();
        for file in disk.edits {
            let one = Changes {
                edits: vec![file.clone()],
                ..Changes::default()
            };
            match vault.apply(&one) {
                Ok(applied) => self.reread(vault, applied.touched(), cx),
                Err(error) => {
                    log::warn!("no se devolvió {}: {error}", file.path);
                    left.push(file.path);
                }
            }
        }
        left
    }
}

#[cfg(test)]
mod tests {
    use metalmind_vault::{NewFile, Rename, TextEdit};
    use pretty_assertions::assert_eq;

    use super::*;

    fn path(path: &str) -> VaultPath {
        VaultPath::new(path).unwrap()
    }

    fn edits(name: &str) -> FileEdits {
        FileEdits {
            path: path(name),
            edits: vec![TextEdit {
                range: 0..2,
                old: "#a".to_owned(),
                new: "#b".to_owned(),
                line: 0,
            }],
            base_len: None,
        }
    }

    /// Solo las ediciones de lo que tiene cambios sin guardar van a su
    /// buffer; los archivos nuevos y los renombres siempre al disco.
    #[test]
    fn las_ediciones_sin_guardar_van_a_su_buffer() {
        let changes = Changes {
            edits: vec![edits("a.md"), edits("b.md"), edits("c.md")],
            creates: vec![NewFile {
                path: path("n.md"),
                text: String::new(),
            }],
            renames: vec![Rename {
                from: path("tags/a.md"),
                to: path("tags/b.md"),
            }],
        };
        let routed = route(&changes, |file| file.as_str() != "b.md");
        assert_eq!(routed.buffers, [edits("a.md"), edits("c.md")]);
        assert_eq!(
            routed.disk,
            Changes {
                edits: vec![edits("b.md")],
                ..changes
            }
        );
    }
}
