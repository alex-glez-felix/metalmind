//! Recorrer el disco con `ignore`. El walk completo y el parcial del watcher
//! usan las mismas reglas, así que reindexar a pedazos da lo mismo que de cero.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};

use ignore::{DirEntry, WalkBuilder, WalkState};

use crate::index::{EntryKind, VaultIndex};
use crate::path::{VaultPath, is_hidden_name};

/// Las reglas del vault, iguales en cualquier máquina: `.gitignore` e
/// `.ignore` del vault aunque no sea un repo git, pero no los de carpetas
/// superiores ni el `excludesFile` global del usuario. Los ocultos van fuera
/// por un filtro propio, porque un `!.obsidian/` en `.gitignore` le gana a
/// `hidden(true)`.
fn walker(root: &Path) -> WalkBuilder {
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(true)
        .parents(false)
        .ignore(true)
        .git_ignore(true)
        .git_exclude(true)
        .git_global(false)
        .require_git(false)
        .follow_links(false);
    builder
}

fn is_visible(entry: &DirEntry) -> bool {
    entry.depth() == 0 || !entry.file_name().to_str().is_some_and(is_hidden_name)
}

/// Lo que el índice guarda de una entrada del walk. Los enlaces simbólicos no
/// se siguen ni se listan: podrían sacar al vault de su raíz.
fn classify(root: &Path, entry: &DirEntry) -> Option<(VaultPath, EntryKind)> {
    if entry.depth() == 0 {
        return None;
    }
    let file_type = entry.file_type()?;
    let Some(path) = VaultPath::from_fs(root, entry.path()) else {
        log::warn!("{} no es una ruta válida del vault", entry.path().display());
        return None;
    };
    if file_type.is_dir() {
        Some((path, EntryKind::Folder))
    } else if file_type.is_file() {
        let kind = EntryKind::of_file(&path);
        Some((path, kind))
    } else {
        None
    }
}

fn accept(root: &Path, result: Result<DirEntry, ignore::Error>) -> Option<(VaultPath, EntryKind)> {
    match result {
        Ok(entry) => classify(root, &entry),
        Err(error) => {
            log::warn!("al recorrer el vault: {error}");
            None
        }
    }
}

/// El índice de todo `root`, recorrido en paralelo.
pub(crate) fn scan(root: &Path) -> VaultIndex {
    let (sender, receiver) = mpsc::channel();
    walker(root)
        .filter_entry(is_visible)
        .build_parallel()
        .run(|| {
            let sender = sender.clone();
            let root = root.to_owned();
            Box::new(move |result| {
                if let Some(found) = accept(&root, result) {
                    // Solo falla si el receptor ya no existe, y vive hasta el final del walk.
                    let _ = sender.send(found);
                }
                WalkState::Continue
            })
        });
    drop(sender);
    VaultIndex::from_entries(receiver.into_iter().collect())
}

/// Lo que hay hoy en disco en `dirty` y en sus carpetas. Recorre desde la raíz
/// para que apliquen todos los `.gitignore` de por medio, pero solo entra en
/// los ancestros de `dirty`.
pub(crate) fn scan_partial(root: &Path, dirty: &[VaultPath]) -> Vec<(VaultPath, EntryKind)> {
    let scope = Arc::new(Scope::new(dirty));
    let root_owned: PathBuf = root.to_owned();
    let filter_scope = Arc::clone(&scope);
    walker(root)
        .filter_entry(move |entry| {
            is_visible(entry)
                && VaultPath::from_fs(&root_owned, entry.path())
                    .is_some_and(|path| filter_scope.allows(&path))
        })
        .build()
        .filter_map(|result| accept(root, result))
        .filter(|(path, _)| scope.contains(path))
        .collect()
}

/// Las rutas sucias y sus ancestros: lo que el walk parcial tiene que pisar.
struct Scope {
    dirty: HashSet<VaultPath>,
    ancestors: HashSet<VaultPath>,
}

impl Scope {
    fn new(dirty: &[VaultPath]) -> Self {
        Self {
            dirty: dirty.iter().cloned().collect(),
            ancestors: dirty.iter().flat_map(VaultPath::ancestors).collect(),
        }
    }

    /// Si `path` es sucia o está dentro de una sucia.
    fn contains(&self, path: &VaultPath) -> bool {
        self.dirty.contains(path)
            || path
                .ancestors()
                .any(|ancestor| self.dirty.contains(&ancestor))
    }

    fn allows(&self, path: &VaultPath) -> bool {
        self.ancestors.contains(path) || self.contains(path)
    }
}
