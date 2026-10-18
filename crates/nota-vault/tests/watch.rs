#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

mod support;

use std::fs;
use std::path::Path;
use std::sync::mpsc::Receiver;

use nota_vault::{EntryKind, Vault, VaultChange, VaultPath, VaultUpdate, write_atomic};
use pretty_assertions::assert_eq;
use proptest::prelude::*;
use support::{disk_listing, listing, make_tree, op_strategy, settle, wait_for_changes};
use tempfile::TempDir;

fn path(path: &str) -> VaultPath {
    VaultPath::new(path).unwrap()
}

fn open(root: &Path) -> (Vault, Receiver<VaultUpdate>) {
    Vault::open(root).unwrap()
}

#[test]
fn crear_y_borrar_un_archivo() {
    let dir = TempDir::new().unwrap();
    let (vault, updates) = open(dir.path());
    fs::write(dir.path().join("a.md"), "").unwrap();
    let changes = wait_for_changes(&updates, |changes| !changes.is_empty());
    assert_eq!(
        changes,
        [VaultChange::Created {
            path: path("a.md"),
            kind: EntryKind::Note
        }]
    );
    fs::remove_file(dir.path().join("a.md")).unwrap();
    let changes = wait_for_changes(&updates, |changes| !changes.is_empty());
    assert_eq!(
        changes,
        [VaultChange::Removed {
            path: path("a.md"),
            kind: EntryKind::Note
        }]
    );
    assert!(vault.snapshot().is_empty());
}

#[test]
fn renombrar_y_mover_un_archivo_es_un_solo_cambio() {
    let dir = TempDir::new().unwrap();
    make_tree(dir.path(), &["a.md", "sub/"]);
    let (vault, updates) = open(dir.path());
    fs::rename(dir.path().join("a.md"), dir.path().join("sub/b.md")).unwrap();
    let changes = wait_for_changes(&updates, |changes| !changes.is_empty());
    assert_eq!(
        changes,
        [VaultChange::Renamed {
            from: path("a.md"),
            to: path("sub/b.md"),
            kind: EntryKind::Note
        }]
    );
    assert_eq!(listing(&vault.snapshot()), ["sub/", "sub/b.md (nota)"]);
}

#[test]
fn renombrar_una_carpeta_mueve_su_contenido() {
    let dir = TempDir::new().unwrap();
    make_tree(dir.path(), &["vieja/a.md", "vieja/hondo/b.png", "otra/"]);
    let (vault, updates) = open(dir.path());
    fs::rename(dir.path().join("vieja"), dir.path().join("otra/nueva")).unwrap();
    let changes = wait_for_changes(&updates, |changes| !changes.is_empty());
    assert_eq!(
        changes,
        [VaultChange::Renamed {
            from: path("vieja"),
            to: path("otra/nueva"),
            kind: EntryKind::Folder
        }]
    );
    assert_eq!(
        listing(&vault.snapshot()),
        [
            "otra/",
            "otra/nueva/",
            "otra/nueva/hondo/",
            "otra/nueva/hondo/b.png (adjunto)",
            "otra/nueva/a.md (nota)",
        ]
    );
}

#[test]
fn sacar_una_carpeta_del_vault_la_borra() {
    let outside = TempDir::new().unwrap();
    let dir = TempDir::new().unwrap();
    make_tree(dir.path(), &["sub/a.md", "b.md"]);
    let (vault, updates) = open(dir.path());
    fs::rename(dir.path().join("sub"), outside.path().join("sub")).unwrap();
    settle(&vault, &updates, &["b.md (nota)"]);
}

#[test]
fn el_guardado_atomico_es_una_modificacion() {
    let dir = TempDir::new().unwrap();
    make_tree(dir.path(), &["nota.md"]);
    let (_vault, updates) = open(dir.path());
    let modified = |changes: &[VaultChange]| {
        changes.iter().any(
            |change| matches!(change, VaultChange::Modified { path } if path.as_str() == "nota.md"),
        )
    };
    let touches_note = |change: &VaultChange| match change {
        VaultChange::Created { path, .. } | VaultChange::Removed { path, .. } => {
            path.as_str() == "nota.md"
        }
        VaultChange::Renamed { from, to, .. } => {
            from.as_str() == "nota.md" || to.as_str() == "nota.md"
        }
        VaultChange::Modified { .. } => false,
    };

    // Temporal oculto, como `write_atomic`.
    write_atomic(&dir.path().join("nota.md"), |out| out.write_all(b"uno")).unwrap();
    let changes = wait_for_changes(&updates, modified);
    assert!(!changes.iter().any(touches_note), "{changes:?}");

    // Temporal visible al lado, como hacen otros editores.
    let temp = dir.path().join("nota.md.tmp");
    fs::write(&temp, "dos").unwrap();
    fs::rename(&temp, dir.path().join("nota.md")).unwrap();
    let changes = wait_for_changes(&updates, modified);
    assert!(!changes.iter().any(touches_note), "{changes:?}");
}

#[test]
fn una_rafaga_llega_en_pocos_lotes_y_queda_igual_que_el_disco() {
    let dir = TempDir::new().unwrap();
    let (vault, updates) = open(dir.path());
    let mut operations = 0;
    for folder in 0..10 {
        let folder = dir.path().join(format!("c{folder}/hondo"));
        fs::create_dir_all(&folder).unwrap();
        for note in 0..30 {
            fs::write(folder.join(format!("n{note}.md")), "").unwrap();
            operations += 1;
        }
    }
    for note in 0..30 {
        let from = dir.path().join(format!("c0/hondo/n{note}.md"));
        if note % 2 == 0 {
            fs::rename(from, dir.path().join(format!("c1/m{note}.md"))).unwrap();
        } else {
            fs::remove_file(from).unwrap();
        }
        operations += 1;
    }
    let batches = settle(&vault, &updates, &disk_listing(dir.path()));
    assert!(
        batches < operations / 10,
        "{batches} lotes para {operations} operaciones"
    );
}

#[test]
fn cambiar_el_gitignore_relee_su_carpeta() {
    let dir = TempDir::new().unwrap();
    make_tree(dir.path(), &["a.md", "privado/b.md"]);
    let (vault, updates) = open(dir.path());
    fs::write(dir.path().join(".gitignore"), "privado/\n").unwrap();
    settle(&vault, &updates, &["a.md (nota)"]);
    fs::remove_file(dir.path().join(".gitignore")).unwrap();
    settle(
        &vault,
        &updates,
        &["privado/", "privado/b.md (nota)", "a.md (nota)"],
    );
}

#[test]
fn lo_oculto_no_genera_cambios() {
    let dir = TempDir::new().unwrap();
    let (vault, updates) = open(dir.path());
    make_tree(
        dir.path(),
        &[".obsidian/workspace.json", ".git/HEAD", ".borrador.md"],
    );
    fs::write(dir.path().join("visible.md"), "").unwrap();
    let changes = wait_for_changes(&updates, |changes| !changes.is_empty());
    assert_eq!(
        changes,
        [VaultChange::Created {
            path: path("visible.md"),
            kind: EntryKind::Note
        }]
    );
    assert_eq!(listing(&vault.snapshot()), ["visible.md (nota)"]);
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 12, ..ProptestConfig::default() })]

    /// Con el watcher de verdad: tras cualquier secuencia de operaciones, el
    /// índice queda igual que el disco.
    #[test]
    fn el_indice_sigue_al_disco(ops in prop::collection::vec(op_strategy(), 1..25)) {
        let dir = TempDir::new().unwrap();
        make_tree(dir.path(), &["a.md", "b/c.md", "b/d/e.png"]);
        let (vault, updates) = open(dir.path());
        for op in &ops {
            op.apply(dir.path());
        }
        settle(&vault, &updates, &disk_listing(dir.path()));
    }
}
