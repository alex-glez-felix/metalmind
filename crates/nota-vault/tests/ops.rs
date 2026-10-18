#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

mod support;

use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

use nota_vault::{FileOpError, Vault, VaultPath};
use pretty_assertions::assert_eq;
use support::{entries, make_tree, settle};
use tempfile::TempDir;

fn path(path: &str) -> VaultPath {
    VaultPath::new(path).unwrap()
}

fn read(root: &Path, path: &str) -> String {
    fs::read_to_string(root.join(path)).unwrap()
}

#[test]
fn crear_nota_y_carpeta() {
    let dir = TempDir::new().unwrap();
    let (vault, updates) = Vault::open(dir.path()).unwrap();
    vault.create_note(&path("nueva/hondo/a.md")).unwrap();
    vault.create_folder(&path("vacia")).unwrap();
    settle(
        &vault,
        &updates,
        &[
            "nueva/",
            "nueva/hondo/",
            "nueva/hondo/a.md (nota)",
            "vacia/",
        ],
    );
    assert_eq!(read(dir.path(), "nueva/hondo/a.md"), "");
}

#[test]
fn crear_no_pisa_lo_que_existe() {
    let dir = TempDir::new().unwrap();
    make_tree(dir.path(), &["a.md", "sub/"]);
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    assert!(matches!(
        vault.create_note(&path("a.md")),
        Err(FileOpError::AlreadyExists(p)) if p == path("a.md")
    ));
    assert!(matches!(
        vault.create_folder(&path("sub")),
        Err(FileOpError::AlreadyExists(_))
    ));
    assert_eq!(read(dir.path(), "a.md"), "a.md");
}

#[test]
fn crear_o_mover_bajo_un_archivo_nombra_el_archivo() {
    let dir = TempDir::new().unwrap();
    make_tree(dir.path(), &["a.md", "b.md"]);
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    let blocked_by_a = |result: Result<(), FileOpError>| matches!(result, Err(FileOpError::NotAFolder(p)) if p == path("a.md"));
    assert!(blocked_by_a(vault.create_note(&path("a.md/x.md"))));
    assert!(blocked_by_a(vault.create_folder(&path("a.md/hondo/c"))));
    assert!(blocked_by_a(
        vault.rename(&path("b.md"), &path("a.md/b.md"))
    ));
    assert_eq!(entries(dir.path()), ["a.md", "b.md"]);
}

#[test]
fn una_nota_tiene_que_ser_md() {
    let dir = TempDir::new().unwrap();
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    assert!(matches!(
        vault.create_note(&path("imagen.png")),
        Err(FileOpError::NotANote(_))
    ));
    assert_eq!(entries(dir.path()), Vec::<String>::new());
}

#[test]
fn renombrar_y_mover() {
    let dir = TempDir::new().unwrap();
    make_tree(dir.path(), &["a.md", "carpeta/b.md"]);
    let (vault, updates) = Vault::open(dir.path()).unwrap();
    vault.rename(&path("a.md"), &path("otra/c.md")).unwrap();
    vault
        .rename(&path("carpeta"), &path("otra/carpeta"))
        .unwrap();
    settle(
        &vault,
        &updates,
        &[
            "otra/",
            "otra/carpeta/",
            "otra/carpeta/b.md (nota)",
            "otra/c.md (nota)",
        ],
    );
    assert_eq!(read(dir.path(), "otra/c.md"), "a.md");
}

#[test]
fn renombrar_no_pisa_el_destino() {
    let dir = TempDir::new().unwrap();
    make_tree(dir.path(), &["a.md", "b.md", "x/", "y/"]);
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    assert!(matches!(
        vault.rename(&path("a.md"), &path("b.md")),
        Err(FileOpError::AlreadyExists(p)) if p == path("b.md")
    ));
    // `rename(2)` sí dejaría pisar una carpeta vacía.
    assert!(matches!(
        vault.rename(&path("x"), &path("y")),
        Err(FileOpError::AlreadyExists(_))
    ));
    assert_eq!(read(dir.path(), "a.md"), "a.md");
    assert_eq!(read(dir.path(), "b.md"), "b.md");
    assert!(dir.path().join("x").is_dir());
}

#[test]
fn renombrar_lo_imposible_falla_sin_tocar_nada() {
    let dir = TempDir::new().unwrap();
    make_tree(dir.path(), &["a/b.md"]);
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    assert!(matches!(
        vault.rename(&path("a"), &path("a/dentro/a")),
        Err(FileOpError::IntoItself(_))
    ));
    assert!(matches!(
        vault.rename(&path("no-existe.md"), &path("c.md")),
        Err(FileOpError::NotFound(_))
    ));
    assert!(matches!(
        vault.rename(&VaultPath::root(), &path("c")),
        Err(FileOpError::Root)
    ));
    assert_eq!(entries(dir.path()), ["a", "a/b.md"]);
}

#[test]
fn borrar_va_a_la_papelera_del_vault() {
    let dir = TempDir::new().unwrap();
    make_tree(
        dir.path(),
        &["a.md", "sub/a.md", "carpeta/x.png", "queda.md"],
    );
    let (vault, updates) = Vault::open(dir.path()).unwrap();
    let first = vault.move_to_trash(&path("a.md")).unwrap();
    let second = vault.move_to_trash(&path("sub/a.md")).unwrap();
    vault.move_to_trash(&path("carpeta")).unwrap();
    assert_eq!(first, vault.root().join(".trash/a.md"));
    assert_eq!(second, vault.root().join(".trash/a 1.md"));
    assert_eq!(read(dir.path(), ".trash/a 1.md"), "sub/a.md");
    assert_eq!(read(dir.path(), ".trash/carpeta/x.png"), "carpeta/x.png");
    settle(&vault, &updates, &["sub/", "queda.md (nota)"]);
}

#[test]
fn borrar_la_raiz_o_lo_que_no_existe_falla() {
    let dir = TempDir::new().unwrap();
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    assert!(matches!(
        vault.move_to_trash(&VaultPath::root()),
        Err(FileOpError::Root)
    ));
    assert!(matches!(
        vault.move_to_trash(&path("no.md")),
        Err(FileOpError::NotFound(_))
    ));
}

#[test]
fn nada_sale_del_vault_por_un_enlace() {
    let outside = TempDir::new().unwrap();
    make_tree(outside.path(), &["ajena.md"]);
    let dir = TempDir::new().unwrap();
    make_tree(dir.path(), &["a.md"]);
    symlink(outside.path(), dir.path().join("enlace")).unwrap();
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    let escapes = |result: Result<_, FileOpError>| matches!(result, Err(FileOpError::Symlink(_)));
    assert!(escapes(vault.create_note(&path("enlace/nueva.md"))));
    assert!(escapes(vault.create_folder(&path("enlace/hondo/c"))));
    assert!(escapes(vault.rename(&path("a.md"), &path("enlace/a.md"))));
    assert!(escapes(
        vault.rename(&path("enlace/ajena.md"), &path("b.md"))
    ));
    assert!(escapes(
        vault.move_to_trash(&path("enlace/ajena.md")).map(drop)
    ));
    assert_eq!(entries(outside.path()), ["ajena.md"]);
    assert_eq!(read(dir.path(), "a.md"), "a.md");
}

#[test]
fn una_papelera_que_es_enlace_no_se_usa() {
    let outside = TempDir::new().unwrap();
    let dir = TempDir::new().unwrap();
    make_tree(dir.path(), &["a.md"]);
    symlink(outside.path(), dir.path().join(".trash")).unwrap();
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    assert!(matches!(
        vault.move_to_trash(&path("a.md")),
        Err(FileOpError::TrashIsSymlink)
    ));
    assert_eq!(read(dir.path(), "a.md"), "a.md");
    assert_eq!(entries(outside.path()), Vec::<String>::new());
}
