#![allow(
    clippy::unwrap_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

mod support;

use std::fs;
use std::path::Path;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use nota_vault::{
    AccessError, ApplyRenameError, FileOpError, LinkIndex, LinkTarget, Vault, VaultPath,
    VaultUpdate, write_atomic,
};
use pretty_assertions::assert_eq;
use tempfile::TempDir;

fn path(path: &str) -> VaultPath {
    VaultPath::new(path).unwrap()
}

fn write(root: &Path, file: &str, text: &str) {
    let full = root.join(file);
    fs::create_dir_all(full.parent().unwrap()).unwrap();
    fs::write(full, text).unwrap();
}

/// El índice de links de un vault recién abierto: el armado de cero.
fn rebuilt(root: &Path) -> LinkIndex {
    LinkIndex::clone(&Vault::open(root).unwrap().0.links())
}

/// Espera a que el índice del vault abierto sea el de abrirlo de nuevo.
fn settle(vault: &Vault, updates: &Receiver<VaultUpdate>) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let expected = rebuilt(vault.root());
        if *vault.links() == expected {
            return;
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if updates.recv_timeout(left).is_err() {
            assert_eq!(
                *vault.links(),
                expected,
                "el índice de links no alcanzó al disco"
            );
        }
    }
}

fn backlinks(index: &LinkIndex, file: &str) -> Vec<String> {
    index
        .backlinks(&path(file))
        .map(|found| format!("{}:{}", found.source, found.link.line))
        .collect()
}

#[test]
fn el_watcher_mantiene_los_links_al_dia() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().canonicalize().unwrap();
    write(&root, "a.md", "[[b]] [[c]]");
    write(&root, "b.md", "");
    let (vault, updates) = Vault::open(&root).unwrap();
    assert_eq!(backlinks(&vault.links(), "b.md"), ["a.md:0"]);

    write(&root, "carpeta/c.md", "[[a]]");
    settle(&vault, &updates);
    assert_eq!(backlinks(&vault.links(), "carpeta/c.md"), ["a.md:0"]);

    write_atomic(&root.join("a.md"), |out| out.write_all(b"\n[[b]]")).unwrap();
    settle(&vault, &updates);
    assert_eq!(backlinks(&vault.links(), "b.md"), ["a.md:1"]);
    assert_eq!(
        backlinks(&vault.links(), "carpeta/c.md"),
        Vec::<String>::new()
    );

    fs::rename(root.join("carpeta"), root.join("otra")).unwrap();
    fs::remove_file(root.join("b.md")).unwrap();
    settle(&vault, &updates);
    let links = vault.links();
    assert_eq!(
        links.outgoing(&path("a.md"))[0].target,
        LinkTarget::Unresolved
    );
    assert_eq!(backlinks(&links, "a.md"), ["otra/c.md:0"]);
}

#[test]
fn renombrar_con_el_plan_no_rompe_ningun_link_y_se_deshace() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let inicio = "[[plan]] [x](viejo/plan.md) `[[plan]]`";
    write(&root, "inicio.md", inicio);
    write(&root, "viejo/plan.md", "[[./hermana]]");
    write(&root, "viejo/hermana.md", "");
    let (vault, updates) = Vault::open(&root).unwrap();
    let before = LinkIndex::clone(&vault.links());
    let plan = vault
        .links()
        .rename_plan(&path("viejo/plan.md"), &path("nuevo/Plan del año.md"))
        .unwrap();
    let applied = vault.apply_rename(&plan).unwrap();
    settle(&vault, &updates);
    assert_eq!(
        fs::read_to_string(root.join("inicio.md")).unwrap(),
        "[[Plan del año]] [x](nuevo/Plan%20del%20año.md) `[[plan]]`"
    );
    assert_eq!(
        fs::read_to_string(root.join("nuevo/Plan del año.md")).unwrap(),
        "[[../viejo/hermana]]"
    );
    assert_eq!(
        backlinks(&vault.links(), "nuevo/Plan del año.md"),
        ["inicio.md:0", "inicio.md:0"]
    );
    assert_eq!(vault.links().unresolved(), []);

    let report = vault.undo_rename(&applied).unwrap();
    assert_eq!(report.restored, [path("inicio.md"), path("viejo/plan.md")]);
    assert_eq!(report.changed, []);
    settle(&vault, &updates);
    assert_eq!(fs::read_to_string(root.join("inicio.md")).unwrap(), inicio);
    assert_eq!(
        fs::read_to_string(root.join("viejo/plan.md")).unwrap(),
        "[[./hermana]]"
    );
    assert_eq!(*vault.links(), before);
}

#[test]
fn deshacer_no_pisa_lo_que_cambio_despues() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().canonicalize().unwrap();
    write(&root, "a.md", "[[plan]]");
    write(&root, "b.md", "[[plan]]");
    write(&root, "plan.md", "");
    let (vault, _updates) = Vault::open(&root).unwrap();
    let plan = vault
        .links()
        .rename_plan(&path("plan.md"), &path("meta.md"))
        .unwrap();
    let applied = vault.apply_rename(&plan).unwrap();
    write(&root, "b.md", "[[meta]] y algo más");
    let report = vault.undo_rename(&applied).unwrap();
    assert_eq!(report.restored, [path("a.md")]);
    assert_eq!(report.changed, [path("b.md")]);
    assert_eq!(fs::read_to_string(root.join("a.md")).unwrap(), "[[plan]]");
    assert_eq!(
        fs::read_to_string(root.join("b.md")).unwrap(),
        "[[meta]] y algo más"
    );
    assert!(root.join("plan.md").is_file());
}

#[test]
fn un_archivo_que_cambio_no_deja_escribir_nada() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().canonicalize().unwrap();
    write(&root, "a.md", "[[plan]]");
    write(&root, "b.md", "[[plan]]");
    write(&root, "plan.md", "");
    let (vault, _updates) = Vault::open(&root).unwrap();
    let plan = vault
        .links()
        .rename_plan(&path("plan.md"), &path("meta.md"))
        .unwrap();
    write(&root, "b.md", "otro texto");
    let error = vault.apply_rename(&plan).unwrap_err();
    assert!(matches!(error, ApplyRenameError::Stale(_)), "{error}");
    assert_eq!(fs::read_to_string(root.join("a.md")).unwrap(), "[[plan]]");
    assert!(root.join("plan.md").is_file());
    assert!(!root.join("meta.md").exists());

    write(&root, "b.md", "[[plan]]");
    write(&root, "meta.md", "");
    let error = vault.apply_rename(&plan).unwrap_err();
    assert!(
        matches!(
            error,
            ApplyRenameError::Access(AccessError::Path(FileOpError::AlreadyExists(_)))
        ),
        "{error}"
    );
    assert_eq!(fs::read_to_string(root.join("a.md")).unwrap(), "[[plan]]");
}
