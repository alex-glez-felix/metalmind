//! El árbol del panel rehace sus filas tras cada cambio y el filtro busca en
//! todo el vault: las dos cosas tienen que ser lineales en las rutas. Lo que
//! cuesta pintar una tecla no depende del largo de la nota. Ver
//! `docs/performance.md`.

#![allow(
    clippy::unwrap_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

use std::collections::BTreeSet;
use std::sync::Arc;

#[path = "support/headless.rs"]
mod headless;

use headless::{Headless, Prepared};
use metalmind_perf::{assert_flat, assert_linear, vault_paths};
use metalmind_ui::FileTree;
use metalmind_vault::{EntryKind, VaultIndex, VaultPath};

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
#[cfg_attr(not(valgrind_available), ignore = "valgrind no está en el PATH")]
fn expandir_y_colapsar_es_lineal_en_las_filas() {
    let folder = VaultPath::new("area-5/tema-5").unwrap();
    assert_linear(8_000, expanded, |mut tree| {
        tree.collapse(&folder);
        tree.expand(&folder);
        tree.rows().len()
    });
}

#[test]
#[cfg_attr(not(valgrind_available), ignore = "valgrind no está en el PATH")]
fn filtrar_es_lineal_en_las_rutas() {
    assert_linear(2_000, expanded, |mut tree| {
        tree.set_filter("tema nota");
        tree.rows().len()
    });
}

/// La nota de `lines` líneas abierta en la ventana, con el cursor a la mitad
/// y en `mode` (`i` para Insert; nada para Normal).
fn in_the_middle(lines: usize, mode: &[&str]) -> Headless {
    let mut headless = Prepared::note(&metalmind_perf::note(lines)).open();
    let line = (lines / 2).to_string();
    let digits: Vec<String> = line.chars().map(String::from).collect();
    let mut keys: Vec<&str> = digits.iter().map(String::as_str).collect();
    keys.push("shift-g");
    keys.extend(mode);
    headless.type_keys(&keys);
    headless
}

/// De 5 k a 80 k líneas, una tecla en Insert cuesta lo mismo hasta su frame:
/// el render solo mira las líneas que se ven.
#[test]
#[cfg_attr(not(valgrind_available), ignore = "valgrind no está en el PATH")]
fn una_tecla_hasta_su_frame_no_depende_del_largo_de_la_nota() {
    assert_flat(
        5_000,
        |lines| in_the_middle(lines, &["i"]),
        |mut headless| {
            headless.key_to_frame("x");
            headless
        },
    );
}

/// `ctrl-d` le da forma a media pantalla de líneas nuevas, no a la nota.
#[test]
#[cfg_attr(not(valgrind_available), ignore = "valgrind no está en el PATH")]
fn ctrl_d_hasta_su_frame_no_depende_del_largo_de_la_nota() {
    assert_flat(
        5_000,
        |lines| in_the_middle(lines, &[]),
        |mut headless| {
            headless.key_to_frame("ctrl-d");
            headless
        },
    );
}
