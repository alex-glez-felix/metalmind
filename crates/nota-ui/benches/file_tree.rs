//! El modelo del árbol del panel con el vault sintético de `nota-perf`
//! (`vault_paths`): 5 k y 50 k notas en 110 carpetas, todas abiertas.
//! Correr con `cargo bench -p nota-ui --bench file_tree`; los números van en
//! `docs/performance.md`.

#![allow(
    clippy::unwrap_used,
    reason = "en un benchmark, un unwrap que falla es un benchmark roto"
)]

use std::collections::BTreeSet;
use std::sync::Arc;

use divan::Bencher;
use nota_ui::FileTree;
use nota_vault::{EntryKind, VaultChange, VaultIndex, VaultPath};

const SIZES: &[usize] = &[5_000, 50_000];

fn main() {
    divan::main();
}

fn path(path: &str) -> VaultPath {
    VaultPath::new(path).unwrap()
}

/// El índice de `nota_perf::vault_paths(notes)`, con sus carpetas;
/// `top` cambia el prefijo de las de arriba, para simular un rename.
fn index(notes: usize, top: &str) -> Arc<VaultIndex> {
    let files: Vec<VaultPath> = nota_perf::vault_paths(notes)
        .iter()
        .map(|file| path(&file.replacen("area", top, 1)))
        .collect();
    let folders: BTreeSet<VaultPath> = files
        .iter()
        .flat_map(VaultPath::ancestors)
        .filter(|folder| !folder.is_root())
        .collect();
    let entries = folders
        .into_iter()
        .map(|folder| (folder, EntryKind::Folder))
        .chain(files.into_iter().map(|file| {
            let kind = if file.name() == "imagen.png" {
                EntryKind::Attachment
            } else {
                EntryKind::Note
            };
            (file, kind)
        }));
    Arc::new(VaultIndex::from_entries(entries))
}

fn folders(index: &VaultIndex) -> Vec<VaultPath> {
    index
        .iter()
        .filter(|entry| entry.kind == EntryKind::Folder)
        .map(|entry| entry.path.clone())
        .collect()
}

fn expanded(notes: usize) -> FileTree {
    let index = index(notes, "area");
    let all = folders(&index);
    FileTree::with_expanded(index, all)
}

#[divan::bench(args = SIZES)]
fn construir_todo_abierto(bencher: Bencher, notes: usize) {
    let index = index(notes, "area");
    let all = folders(&index);
    bencher.bench(|| FileTree::with_expanded(Arc::clone(&index), all.iter().cloned()));
}

/// Cerrar y volver a abrir una carpeta con todo lo demás abierto.
#[divan::bench(args = SIZES)]
fn expandir_y_colapsar(bencher: Bencher, notes: usize) {
    let mut tree = expanded(notes);
    let folder = path("area-5/tema-5");
    bencher.bench_local(|| {
        tree.collapse(&folder);
        tree.expand(&folder);
    });
}

#[divan::bench(args = SIZES)]
fn bajar_una_fila(bencher: Bencher, notes: usize) {
    let mut tree = expanded(notes);
    tree.select_row(notes / 2);
    bencher.bench_local(|| {
        tree.select_next();
        tree.select_previous();
    });
}

/// Poner el filtro y quitarlo: lo que cuesta una tecla en `/`.
#[divan::bench(args = SIZES, consts = [0, 1])]
fn filtrar<const PEOR: usize>(bencher: Bencher, notes: usize) {
    let query = if PEOR == 1 { "tema nota" } else { "a3t7n42" };
    let mut tree = expanded(notes);
    bencher.bench_local(|| {
        tree.set_filter(query);
        tree.set_filter("");
    });
}

/// Un lote del watcher con el rename de una carpeta de arriba, ida y vuelta.
#[divan::bench(args = SIZES)]
fn aplicar_un_rename(bencher: Bencher, notes: usize) {
    let (before, after) = (index(notes, "area"), index(notes, "zona"));
    let rename = |from: &str, to: &str| {
        (0..10)
            .map(|top| VaultChange::Renamed {
                from: path(&format!("{from}-{top}")),
                to: path(&format!("{to}-{top}")),
                kind: EntryKind::Folder,
            })
            .collect::<Vec<_>>()
    };
    let (there, back) = (rename("area", "zona"), rename("zona", "area"));
    let mut tree = expanded(notes);
    bencher.bench_local(|| {
        tree.apply(&there, Arc::clone(&after));
        tree.apply(&back, Arc::clone(&before));
    });
}
