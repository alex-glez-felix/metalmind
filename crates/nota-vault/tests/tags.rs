//! Los tags de un vault de verdad: se indexan al abrir y siguen al disco por
//! el watcher, igual que el índice de archivos.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

mod support;

use std::fs;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use nota_vault::tags::{TagKey, TagQuery, Tags};
use nota_vault::{Vault, VaultOptions, VaultPath, VaultUpdate};
use pretty_assertions::assert_eq;
use support::make_tree;
use tempfile::TempDir;

fn notes(tags: &Tags, query: &str) -> Vec<String> {
    TagQuery::parse(query)
        .query
        .notes(tags)
        .into_iter()
        .map(ToString::to_string)
        .collect()
}

/// Espera a que los tags publicados cumplan `done`.
fn wait_for_tags(vault: &Vault, updates: &Receiver<VaultUpdate>, done: impl Fn(&Tags) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !done(&vault.tags()) {
        let left = deadline.saturating_duration_since(Instant::now());
        assert!(
            updates.recv_timeout(left).is_ok(),
            "los tags no alcanzaron al disco"
        );
    }
}

#[test]
fn los_tags_se_indexan_al_abrir() {
    let dir = TempDir::new().unwrap();
    make_tree(dir.path(), &["tags/", "x.md", "y.md"]);
    fs::write(dir.path().join("tags/rust.md"), "---\naliases: [rs]\n---\n").unwrap();
    fs::write(dir.path().join("x.md"), "#rs y #nota").unwrap();
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    assert_eq!(notes(&vault.tags(), "#rust"), ["x.md"]);
    assert_eq!(notes(&vault.tags(), "-#"), ["y.md"]);
}

#[test]
fn los_tags_siguen_al_disco() {
    let dir = TempDir::new().unwrap();
    let (vault, updates) = Vault::open(dir.path()).unwrap();
    fs::write(dir.path().join("a.md"), "#rust").unwrap();
    wait_for_tags(&vault, &updates, |tags| notes(tags, "#rust") == ["a.md"]);
    fs::create_dir(dir.path().join("tags")).unwrap();
    fs::write(dir.path().join("tags/lenguajes.md"), "").unwrap();
    fs::write(
        dir.path().join("tags/rust.md"),
        "---\nparents: [lenguajes]\n---\n",
    )
    .unwrap();
    wait_for_tags(&vault, &updates, |tags| {
        notes(tags, "#lenguajes") == ["a.md"]
    });
    fs::rename(dir.path().join("a.md"), dir.path().join("b.md")).unwrap();
    wait_for_tags(&vault, &updates, |tags| {
        notes(tags, "#lenguajes") == ["b.md"]
    });
    fs::write(dir.path().join("b.md"), "sin tags").unwrap();
    wait_for_tags(&vault, &updates, |tags| notes(tags, "-#") == ["b.md"]);
}

#[test]
fn cada_update_dice_que_tags_cambiaron() {
    let dir = TempDir::new().unwrap();
    let (_vault, updates) = Vault::open(dir.path()).unwrap();
    fs::write(dir.path().join("a.md"), "#Rust").unwrap();
    let update = updates.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(update.tag_changes, [TagKey::new("rust")]);
    assert_eq!(update.tags.display_name(&TagKey::new("rust")), Some("Rust"));
}

#[test]
fn la_carpeta_de_tags_se_configura() {
    let dir = TempDir::new().unwrap();
    make_tree(dir.path(), &["etiquetas/rust.md", "tags/gpui.md"]);
    let options = VaultOptions {
        tags_folder: VaultPath::new("etiquetas").unwrap(),
    };
    let (vault, _updates) = Vault::open_with(dir.path(), options).unwrap();
    let tags = vault.tags();
    assert!(tags.graph().page(&TagKey::new("rust")).is_some());
    assert!(tags.graph().page(&TagKey::new("gpui")).is_none());
    assert_eq!(notes(&tags, "-#"), ["tags/gpui.md"]);
}
