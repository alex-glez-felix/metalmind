#![allow(
    clippy::unwrap_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

mod support;

use std::fs;
use std::path::Path;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use nota_vault::{LinkIndex, LinkTarget, Vault, VaultPath, VaultUpdate, read, write_atomic};
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
fn renombrar_con_el_plan_no_rompe_ningun_link() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().canonicalize().unwrap();
    write(&root, "inicio.md", "[[plan]] [x](viejo/plan.md) `[[plan]]`");
    write(&root, "viejo/plan.md", "[[./hermana]]");
    write(&root, "viejo/hermana.md", "");
    let (vault, updates) = Vault::open(&root).unwrap();
    let plan = vault
        .links()
        .rename_plan(&path("viejo/plan.md"), &path("nuevo/Plan del año.md"))
        .unwrap();
    for file in &plan.files {
        let full = root.join(file.path.as_str());
        let (text, _) = file.apply(&read(&full).unwrap().unwrap()).unwrap();
        write_atomic(&full, |out| out.write_all(text.as_bytes())).unwrap();
    }
    vault.rename(&plan.from, &plan.to).unwrap();
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
}
