//! El modelo del árbol del panel con un vault sintético de 5 000 notas,
//! el mismo de `nota-vault/benches/open.rs`. Correr con
//! `cargo bench -p nota-ui --bench file_tree`.

#![allow(
    clippy::unwrap_used,
    reason = "en un benchmark, un unwrap que falla es un benchmark roto"
)]

use std::sync::Arc;

use divan::Bencher;
use nota_ui::FileTree;
use nota_vault::{EntryKind, VaultChange, VaultIndex, VaultPath};

const NOTES: usize = 5_000;

fn main() {
    divan::main();
}

fn path(path: &str) -> VaultPath {
    VaultPath::new(path).unwrap()
}

/// 10 carpetas × 10 subcarpetas × 50 notas, más un adjunto por subcarpeta;
/// `top` cambia el nombre de las carpetas de arriba, para simular un rename.
fn entries(top: &str) -> Vec<(VaultPath, EntryKind)> {
    let mut entries = Vec::new();
    for area in 0..10 {
        let area_dir = format!("{top}-{area}");
        entries.push((path(&area_dir), EntryKind::Folder));
        for topic in 0..10 {
            let dir = format!("{area_dir}/tema-{topic}");
            entries.push((path(&dir), EntryKind::Folder));
            for note in 0..NOTES / 100 {
                entries.push((path(&format!("{dir}/nota-{note}.md")), EntryKind::Note));
            }
            entries.push((path(&format!("{dir}/imagen.png")), EntryKind::Attachment));
        }
    }
    entries
}

fn index() -> Arc<VaultIndex> {
    Arc::new(VaultIndex::from_entries(entries("area")))
}

fn folders(index: &VaultIndex) -> Vec<VaultPath> {
    index
        .iter()
        .filter(|entry| entry.kind == EntryKind::Folder)
        .map(|entry| entry.path.clone())
        .collect()
}

/// Todas las carpetas abiertas: 5 210 filas visibles.
fn expanded() -> FileTree {
    let index = index();
    let all = folders(&index);
    let tree = FileTree::with_expanded(index, all);
    assert_eq!(tree.rows().len(), NOTES + 210);
    tree
}

#[divan::bench]
fn construir_cerrado(bencher: Bencher) {
    let index = index();
    bencher.bench(|| FileTree::new(Arc::clone(&index)));
}

#[divan::bench]
fn construir_todo_abierto(bencher: Bencher) {
    let index = index();
    let all = folders(&index);
    bencher.bench(|| FileTree::with_expanded(Arc::clone(&index), all.iter().cloned()));
}

/// Abrir y cerrar una carpeta con todo lo demás abierto: rehace las filas.
#[divan::bench]
fn expandir_y_colapsar_todo_abierto(bencher: Bencher) {
    let mut tree = expanded();
    let folder = path("area-5/tema-5");
    bencher.bench_local(|| {
        tree.collapse(&folder);
        tree.expand(&folder);
    });
}

#[divan::bench]
fn bajar_una_fila(bencher: Bencher) {
    let mut tree = expanded();
    tree.select_row(NOTES / 2);
    bencher.bench_local(|| {
        tree.select_next();
        tree.select_previous();
    });
}

#[divan::bench(args = ["n", "tema nota", "a3t7n42"])]
fn filtrar(bencher: Bencher, query: &str) {
    let mut tree = expanded();
    bencher.bench_local(|| {
        tree.set_filter(query);
        tree.set_filter("");
    });
}

/// Un lote del watcher con el rename de una carpeta de arriba (510 rutas).
#[divan::bench]
fn aplicar_un_rename(bencher: Bencher) {
    let before = index();
    let mut renamed = entries("area");
    for (entry, _) in &mut renamed {
        *entry = entry
            .rebase(&path("area-0"), &path("zona-0"))
            .unwrap_or_else(|| entry.clone());
    }
    let after = Arc::new(VaultIndex::from_entries(renamed));
    let there = [VaultChange::Renamed {
        from: path("area-0"),
        to: path("zona-0"),
        kind: EntryKind::Folder,
    }];
    let back = [VaultChange::Renamed {
        from: path("zona-0"),
        to: path("area-0"),
        kind: EntryKind::Folder,
    }];
    let mut tree = expanded();
    bencher.bench_local(|| {
        tree.apply(&there, Arc::clone(&after));
        tree.apply(&back, Arc::clone(&before));
    });
}
