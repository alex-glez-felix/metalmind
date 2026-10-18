#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

mod support;

use std::fs;
use std::os::unix::fs::symlink;

use nota_vault::{EntryKind, Vault, VaultError, VaultPath};
use pretty_assertions::assert_eq;
use support::{listing, make_tree};
use tempfile::TempDir;

#[test]
fn indexa_notas_adjuntos_y_carpetas() {
    let dir = TempDir::new().unwrap();
    make_tree(
        dir.path(),
        &["a.md", "img.png", "sub/b.md", "sub/hondo/c.MD", "vacia/"],
    );
    let vault = Vault::open(dir.path()).unwrap();
    assert_eq!(
        listing(&vault.snapshot()),
        [
            "sub/",
            "sub/hondo/",
            "sub/hondo/c.MD (nota)",
            "sub/b.md (nota)",
            "vacia/",
            "a.md (nota)",
            "img.png (adjunto)",
        ]
    );
}

#[test]
fn ignora_siempre_obsidian_git_trash_y_ocultos() {
    let dir = TempDir::new().unwrap();
    make_tree(
        dir.path(),
        &[
            "a.md",
            ".obsidian/app.json",
            ".git/HEAD",
            ".trash/vieja.md",
            ".oculta.md",
            "sub/.borrador/x.md",
        ],
    );
    // Ni volviéndolas a incluir desde .gitignore.
    fs::write(dir.path().join(".gitignore"), "!.obsidian/\n!.oculta.md\n").unwrap();
    let vault = Vault::open(dir.path()).unwrap();
    assert_eq!(listing(&vault.snapshot()), ["sub/", "a.md (nota)"]);
}

#[test]
fn respeta_gitignore_e_ignore_aunque_no_sea_un_repo() {
    let dir = TempDir::new().unwrap();
    make_tree(
        dir.path(),
        &[
            "a.md",
            "build/x.md",
            "sub/b.md",
            "sub/c.log",
            "privado/d.md",
        ],
    );
    fs::write(dir.path().join(".gitignore"), "build/\n").unwrap();
    fs::write(dir.path().join("sub/.gitignore"), "*.log\n").unwrap();
    fs::write(dir.path().join(".ignore"), "privado/\n").unwrap();
    let vault = Vault::open(dir.path()).unwrap();
    assert_eq!(
        listing(&vault.snapshot()),
        ["sub/", "sub/b.md (nota)", "a.md (nota)"]
    );
}

#[test]
fn no_lee_ignores_de_fuera_del_vault() {
    let outer = TempDir::new().unwrap();
    fs::write(outer.path().join(".gitignore"), "*.md\n").unwrap();
    make_tree(outer.path(), &["vault/a.md"]);
    let vault = Vault::open(outer.path().join("vault")).unwrap();
    assert_eq!(listing(&vault.snapshot()), ["a.md (nota)"]);
}

#[test]
fn no_sigue_ni_lista_enlaces_simbolicos() {
    let outside = TempDir::new().unwrap();
    make_tree(outside.path(), &["secreto.md", "carpeta/x.md"]);
    let dir = TempDir::new().unwrap();
    make_tree(dir.path(), &["a.md"]);
    symlink(
        outside.path().join("secreto.md"),
        dir.path().join("enlace.md"),
    )
    .unwrap();
    symlink(outside.path().join("carpeta"), dir.path().join("enlace")).unwrap();
    let vault = Vault::open(dir.path()).unwrap();
    assert_eq!(listing(&vault.snapshot()), ["a.md (nota)"]);
}

#[test]
fn la_raiz_puede_ser_un_enlace() {
    let dir = TempDir::new().unwrap();
    make_tree(dir.path(), &["real/a.md"]);
    symlink(dir.path().join("real"), dir.path().join("enlace")).unwrap();
    let vault = Vault::open(dir.path().join("enlace")).unwrap();
    assert_eq!(
        vault.root(),
        dir.path().join("real").canonicalize().unwrap()
    );
    assert_eq!(listing(&vault.snapshot()), ["a.md (nota)"]);
}

#[test]
fn abrir_algo_que_no_es_carpeta_falla() {
    let dir = TempDir::new().unwrap();
    make_tree(dir.path(), &["a.md"]);
    assert!(matches!(
        Vault::open(dir.path().join("a.md")),
        Err(VaultError::NotADirectory(_))
    ));
    assert!(matches!(
        Vault::open(dir.path().join("no-existe")),
        Err(VaultError::Open { .. })
    ));
}

#[test]
fn el_arbol_se_recorre_desde_la_raiz() {
    let dir = TempDir::new().unwrap();
    make_tree(dir.path(), &["b/x.md", "a.md"]);
    let index = Vault::open(dir.path()).unwrap().snapshot();
    let root: Vec<_> = index
        .children(&VaultPath::root())
        .map(|entry| (entry.path.as_str(), entry.kind))
        .collect();
    assert_eq!(root, [("b", EntryKind::Folder), ("a.md", EntryKind::Note)]);
    let b = VaultPath::new("b").unwrap();
    assert_eq!(index.children(&b).count(), 1);
    assert_eq!(index.files().count(), 2);
    assert_eq!(
        index.kind(&VaultPath::new("b/x.md").unwrap()),
        Some(EntryKind::Note)
    );
}
