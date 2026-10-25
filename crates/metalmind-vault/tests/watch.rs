#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

mod support;

use std::fs;
use std::path::Path;
use std::sync::mpsc::Receiver;

use metalmind_vault::{EntryKind, Vault, VaultChange, VaultPath, VaultUpdate, write_atomic};
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
fn marcar_una_carpeta_como_cache_la_saca_del_vault() {
    let dir = TempDir::new().unwrap();
    make_tree(dir.path(), &["a.md", "target/debug/b.md"]);
    let (vault, updates) = open(dir.path());
    fs::write(
        dir.path().join("target/CACHEDIR.TAG"),
        "Signature: 8a477f597d28d172789f06886806bc55",
    )
    .unwrap();
    settle(&vault, &updates, &["a.md (nota)"]);
    assert_eq!(inotify_watches(), 1, "solo la raíz");
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

/// Los watches de inotify de este proceso. nextest corre cada test en su
/// propio proceso, así que son los del vault del test.
fn inotify_watches() -> usize {
    fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|fd| {
            let fd = fd.ok()?;
            let target = fs::read_link(fd.path()).ok()?;
            (target == Path::new("anon_inode:inotify")).then(|| fd.file_name())
        })
        .map(|fd| {
            let info = fs::read_to_string(Path::new("/proc/self/fdinfo").join(fd)).unwrap();
            info.lines()
                .filter(|line| line.starts_with("inotify wd:"))
                .count()
        })
        .sum()
}

/// #97: un vault con un proyecto de Node adentro no gasta un watch por
/// carpeta de `node_modules`.
#[test]
fn solo_vigila_las_carpetas_del_vault() {
    let dir = TempDir::new().unwrap();
    let mut tree = vec!["notas/sub/a.md", ".cache/x/y/z", "build/x/y/z.md"];
    let deps: Vec<String> = (0..40)
        .map(|n| format!("web/node_modules/dep{n}/lib/index.js"))
        .collect();
    tree.extend(deps.iter().map(String::as_str));
    make_tree(dir.path(), &tree);
    fs::write(dir.path().join(".gitignore"), "build/\n").unwrap();
    let (vault, updates) = open(dir.path());
    let expected = ["notas/", "notas/sub/", "notas/sub/a.md (nota)", "web/"];
    assert_eq!(listing(&vault.snapshot()), expected);
    assert_eq!(inotify_watches(), 4, "la raíz, notas, notas/sub y web");

    fs::remove_dir_all(dir.path().join("notas/sub")).unwrap();
    settle(&vault, &updates, &["notas/", "web/"]);
    assert_eq!(inotify_watches(), 3);
    assert_eq!(vault.unwatched(), None);
}

#[test]
fn una_carpeta_nueva_se_vigila_sin_perder_lo_que_ya_tenia() {
    let dir = TempDir::new().unwrap();
    let (vault, updates) = open(dir.path());
    // Antes de que el vault la vea: este archivo no da evento.
    make_tree(dir.path(), &["a/b/c.md"]);
    settle(&vault, &updates, &["a/", "a/b/", "a/b/c.md (nota)"]);
    fs::write(dir.path().join("a/b/d.md"), "").unwrap();
    settle(
        &vault,
        &updates,
        &["a/", "a/b/", "a/b/c.md (nota)", "a/b/d.md (nota)"],
    );
    fs::rename(dir.path().join("a/b"), dir.path().join("e")).unwrap();
    settle(
        &vault,
        &updates,
        &["a/", "e/", "e/c.md (nota)", "e/d.md (nota)"],
    );
    fs::write(dir.path().join("e/f.md"), "").unwrap();
    settle(
        &vault,
        &updates,
        &[
            "a/",
            "e/",
            "e/c.md (nota)",
            "e/d.md (nota)",
            "e/f.md (nota)",
        ],
    );
    assert_eq!(inotify_watches(), 3);
}

#[test]
fn una_carpeta_reemplazada_en_un_lote_se_sigue_vigilando() {
    let dir = TempDir::new().unwrap();
    make_tree(dir.path(), &["a/vieja.md"]);
    let (vault, updates) = open(dir.path());
    fs::remove_dir_all(dir.path().join("a")).unwrap();
    make_tree(dir.path(), &["a/nueva.md"]);
    settle(&vault, &updates, &["a/", "a/nueva.md (nota)"]);
    fs::write(dir.path().join("a/otra.md"), "").unwrap();
    settle(
        &vault,
        &updates,
        &["a/", "a/nueva.md (nota)", "a/otra.md (nota)"],
    );
    assert_eq!(inotify_watches(), 2);
}

#[test]
fn una_carpeta_que_sale_del_vault_suelta_sus_watches() {
    let outside = TempDir::new().unwrap();
    let dir = TempDir::new().unwrap();
    make_tree(dir.path(), &["sub/deep/a.md", "otra/b.md", "web/"]);
    let (vault, updates) = open(dir.path());
    assert_eq!(inotify_watches(), 5);
    fs::rename(dir.path().join("sub"), outside.path().join("sub")).unwrap();
    fs::rename(dir.path().join("otra"), dir.path().join("web/node_modules")).unwrap();
    settle(&vault, &updates, &["web/"]);
    assert_eq!(inotify_watches(), 2, "la raíz y web");
    fs::write(outside.path().join("sub/deep/c.md"), "").unwrap();
    fs::write(dir.path().join("web/node_modules/c.md"), "").unwrap();
    make_tree(dir.path(), &["d.md"]);
    settle(&vault, &updates, &["web/", "d.md (nota)"]);
}

#[test]
fn el_exclude_de_git_relee_todo_aunque_git_llegue_despues() {
    let dir = TempDir::new().unwrap();
    make_tree(dir.path(), &["a.md", "privado/b.md"]);
    let (vault, updates) = open(dir.path());
    fs::create_dir_all(dir.path().join(".git/info")).unwrap();
    // Que el vault vea `.git/info` antes de escribir en ella.
    make_tree(dir.path(), &["c.md"]);
    settle(
        &vault,
        &updates,
        &[
            "privado/",
            "privado/b.md (nota)",
            "a.md (nota)",
            "c.md (nota)",
        ],
    );
    fs::write(dir.path().join(".git/info/exclude"), "privado/\n").unwrap();
    settle(&vault, &updates, &["a.md (nota)", "c.md (nota)"]);
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
