//! Lo que comparten los tests del vault.

#![allow(
    dead_code,
    reason = "cada archivo de tests usa una parte distinta de support"
)]

pub mod history;
pub mod links;

use std::fs;
use std::path::Path;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use nota_vault::{EntryKind, Vault, VaultChange, VaultIndex, VaultPath, VaultUpdate, write_atomic};
use pretty_assertions::assert_eq;
use proptest::prelude::*;

/// Crea archivos (con su carpeta) y, si la ruta acaba en `/`, carpetas vacías.
pub fn make_tree(root: &Path, paths: &[&str]) {
    for path in paths {
        let full = root.join(path);
        if path.ends_with('/') {
            fs::create_dir_all(&full).unwrap();
        } else {
            fs::create_dir_all(full.parent().unwrap()).unwrap();
            fs::write(&full, path.as_bytes()).unwrap();
        }
    }
}

/// El árbol en orden de recorrido (profundidad primero), legible en un diff.
pub fn listing(index: &VaultIndex) -> Vec<String> {
    let mut lines = Vec::new();
    walk(index, &VaultPath::root(), &mut lines);
    lines
}

fn walk(index: &VaultIndex, dir: &VaultPath, lines: &mut Vec<String>) {
    for entry in index.children(dir) {
        lines.push(match entry.kind {
            EntryKind::Folder => format!("{}/", entry.path),
            EntryKind::Note => format!("{} (nota)", entry.path),
            EntryKind::Attachment => format!("{} (adjunto)", entry.path),
        });
        walk(index, entry.path, lines);
    }
}

/// Lo que debería mostrar el índice según el disco, leído con `std::fs` y sin
/// `ignore`: solo vale para árboles sin `.gitignore`.
pub fn disk_listing(root: &Path) -> Vec<String> {
    let mut lines = Vec::new();
    disk_walk(root, "", &mut lines);
    lines
}

fn disk_walk(dir: &Path, prefix: &str, lines: &mut Vec<String>) {
    let mut children: Vec<(bool, String, String)> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|entry| {
            let entry = entry.unwrap();
            let name = entry.file_name().into_string().unwrap();
            let file_type = entry.file_type().unwrap();
            let visible = !name.starts_with('.') && (file_type.is_dir() || file_type.is_file());
            visible.then(|| (file_type.is_file(), name.to_lowercase(), name))
        })
        .collect();
    children.sort();
    for (is_file, _, name) in children {
        let path = format!("{prefix}{name}");
        if is_file {
            let kind = if name.to_lowercase().ends_with(".md") && name.len() > 3 {
                "nota"
            } else {
                "adjunto"
            };
            lines.push(format!("{path} ({kind})"));
        } else {
            lines.push(format!("{path}/"));
            disk_walk(&dir.join(&name), &format!("{path}/"), lines);
        }
    }
}

/// Lo bastante para cualquier CI; si se acaba, el watcher perdió algo.
const TIMEOUT: Duration = Duration::from_secs(10);

/// Junta cambios hasta que `done` diga que ya llegó lo esperado.
pub fn wait_for_changes(
    updates: &Receiver<VaultUpdate>,
    done: impl Fn(&[VaultChange]) -> bool,
) -> Vec<VaultChange> {
    let deadline = Instant::now() + TIMEOUT;
    let mut changes = Vec::new();
    while !done(&changes) {
        let left = deadline.saturating_duration_since(Instant::now());
        let update = updates
            .recv_timeout(left)
            .unwrap_or_else(|_| panic!("sin el cambio esperado; llegaron {changes:?}"));
        changes.extend(update.changes);
    }
    changes
}

/// Espera a que el índice muestre `expected` y devuelve cuántos lotes llegaron.
pub fn settle(
    vault: &Vault,
    updates: &Receiver<VaultUpdate>,
    expected: &[impl AsRef<str>],
) -> usize {
    let expected: Vec<&str> = expected.iter().map(AsRef::as_ref).collect();
    let deadline = Instant::now() + TIMEOUT;
    let mut batches = 0;
    while listing(&vault.snapshot()) != expected {
        let left = deadline.saturating_duration_since(Instant::now());
        if updates.recv_timeout(left).is_err() {
            assert_eq!(
                listing(&vault.snapshot()),
                expected,
                "el índice no alcanzó al disco"
            );
        }
        batches += 1;
    }
    batches
}

/// Una operación sobre el disco para los proptests. Las que no se pueden hacer
/// (origen que no existe, carpeta que es archivo) no hacen nada: manda el disco.
#[derive(Debug, Clone)]
pub enum Op {
    WriteFile(String),
    CreateDir(String),
    Remove(String),
    Rename(String, String),
    SaveAtomic(String),
}

pub fn op_strategy() -> impl Strategy<Value = Op> {
    let path = prop::collection::vec(
        prop::sample::select(vec!["a", "b", "x.md", "Y.md", "z.png", ".h"]),
        1..=3,
    )
    .prop_map(|parts| parts.join("/"));
    prop_oneof![
        path.clone().prop_map(Op::WriteFile),
        path.clone().prop_map(Op::CreateDir),
        path.clone().prop_map(Op::Remove),
        (path.clone(), path.clone()).prop_map(|(from, to)| Op::Rename(from, to)),
        path.prop_map(Op::SaveAtomic),
    ]
}

impl Op {
    pub fn apply(&self, root: &Path) {
        let at = |path: &str| root.join(path);
        let parent = |path: &str| {
            let _ = fs::create_dir_all(at(path).parent().unwrap());
        };
        let _ = match self {
            Self::WriteFile(path) => {
                parent(path);
                fs::write(at(path), "")
            }
            Self::CreateDir(path) => fs::create_dir_all(at(path)),
            Self::Remove(path) => {
                fs::remove_file(at(path)).or_else(|_| fs::remove_dir_all(at(path)))
            }
            Self::Rename(from, to) => {
                parent(to);
                fs::rename(at(from), at(to))
            }
            Self::SaveAtomic(path) if at(path).is_file() => {
                write_atomic(&at(path), |out| out.write_all(b"guardado"))
                    .map_err(std::io::Error::other)
            }
            Self::SaveAtomic(_) => Ok(()),
        };
    }
}

/// Todo lo que hay bajo `root`, ocultos incluidos, ordenado.
pub fn entries(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    collect_entries(root, root, &mut found);
    found.sort();
    found
}

fn collect_entries(root: &Path, dir: &Path, found: &mut Vec<String>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        found.push(
            path.strip_prefix(root)
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned(),
        );
        if path.is_dir() && !path.is_symlink() {
            collect_entries(root, &path, found);
        }
    }
}
