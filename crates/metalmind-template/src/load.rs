//! Leer del vault las plantillas. Lo único de este crate que toca el disco.

use std::path::Path;

use metalmind_vault::{EntryKind, FileError, VaultIndex, VaultPath, read};

use crate::draft::Templates;

/// Lo leído, más los archivos que no se pudieron leer.
#[derive(Debug)]
pub struct Loaded<T> {
    pub value: T,
    pub errors: Vec<FileError>,
}

impl Templates {
    /// Las notas de `folder` y sus subcarpetas, según `index`.
    #[must_use]
    pub fn load(root: &Path, index: &VaultIndex, folder: &VaultPath) -> Loaded<Self> {
        let mut notes = Vec::new();
        collect_notes(index, folder, &mut notes);
        let mut errors = Vec::new();
        let mut templates = Vec::new();
        for path in notes {
            let name = &path.as_str()[folder_prefix(folder)..];
            if let Some(text) = read_note(root, &path, &mut errors) {
                templates.push((stem(name).to_owned(), text));
            }
        }
        Loaded {
            value: Self::new(templates),
            errors,
        }
    }
}

fn collect_notes(index: &VaultIndex, dir: &VaultPath, notes: &mut Vec<VaultPath>) {
    for entry in index.children(dir) {
        match entry.kind {
            EntryKind::Folder => collect_notes(index, entry.path, notes),
            EntryKind::Note => notes.push(entry.path.clone()),
            EntryKind::Attachment => {}
        }
    }
}

fn folder_prefix(folder: &VaultPath) -> usize {
    if folder.is_root() {
        0
    } else {
        folder.as_str().len() + 1
    }
}

/// Sin la extensión `.md`, que en una nota del índice siempre está.
fn stem(name: &str) -> &str {
    name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}

/// `None` si no se pudo leer o ya no existe: el índice puede ir un paso
/// detrás del disco.
fn read_note(root: &Path, path: &VaultPath, errors: &mut Vec<FileError>) -> Option<String> {
    match read(&root.join(path.as_str())) {
        Ok(text) => text,
        Err(error) => {
            errors.push(error);
            None
        }
    }
}
