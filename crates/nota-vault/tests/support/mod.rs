//! Lo que comparten los tests del vault.

#![allow(
    dead_code,
    reason = "cada archivo de tests usa una parte distinta de support"
)]

use std::fs;
use std::path::Path;

use nota_vault::{EntryKind, VaultIndex, VaultPath};

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
