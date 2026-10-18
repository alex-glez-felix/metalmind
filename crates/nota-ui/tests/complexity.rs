//! El árbol del panel rehace sus filas tras cada cambio y el filtro busca en
//! todo el vault: las dos cosas tienen que ser lineales en las rutas. Ver
//! `docs/performance.md`.

#![allow(
    clippy::unwrap_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

use std::collections::BTreeSet;
use std::sync::Arc;

use nota_perf::{assert_linear, vault_paths};
use nota_ui::FileTree;
use nota_vault::{EntryKind, VaultIndex, VaultPath};

/// El vault sintético con todas sus carpetas abiertas.
fn expanded(notes: usize) -> FileTree {
    let files: Vec<VaultPath> = vault_paths(notes)
        .iter()
        .map(|file| VaultPath::new(file).unwrap())
        .collect();
    let folders: BTreeSet<VaultPath> = files
        .iter()
        .flat_map(VaultPath::ancestors)
        .filter(|folder| !folder.is_root())
        .collect();
    let entries = folders
        .iter()
        .map(|folder| (folder.clone(), EntryKind::Folder))
        .chain(files.into_iter().map(|file| (file, EntryKind::Note)));
    FileTree::with_expanded(Arc::new(VaultIndex::from_entries(entries)), folders)
}

#[test]
fn expandir_y_colapsar_es_lineal_en_las_filas() {
    let folder = VaultPath::new("area-5/tema-5").unwrap();
    assert_linear(8_000, expanded, |mut tree| {
        tree.collapse(&folder);
        tree.expand(&folder);
        tree.rows().len()
    });
}

#[test]
fn filtrar_es_lineal_en_las_rutas() {
    assert_linear(2_000, expanded, |mut tree| {
        tree.set_filter("tema nota");
        tree.rows().len()
    });
}
