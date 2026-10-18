//! Aplicar y deshacer en disco los planes de tags (05 §5, §9): cada archivo
//! cambia solo en lo planeado y deshacer deja el vault byte a byte como estaba.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use nota_vault::tags::{PageChange, TagColor};
use nota_vault::{AccessError, ApplyError, FileOpError, Vault, VaultPath};
use pretty_assertions::assert_eq;
use tempfile::TempDir;

/// Cada archivo y carpeta bajo `root` con su contenido, en orden.
fn contents(root: &Path) -> BTreeMap<String, Option<String>> {
    let mut found = BTreeMap::new();
    let mut pending = vec![root.to_owned()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path.strip_prefix(root).unwrap().display().to_string();
            if path.is_dir() {
                found.insert(format!("{name}/"), None);
                pending.push(path);
            } else {
                found.insert(name, Some(fs::read_to_string(&path).unwrap()));
            }
        }
    }
    found
}

fn write(root: &Path, files: &[(&str, &str)]) {
    for (file, text) in files {
        let path = root.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
}

fn path(path: &str) -> VaultPath {
    VaultPath::new(path).unwrap()
}

const RENAME: &[(&str, &str)] = &[
    (
        "tags/a.md",
        "---\ncolor: rojo\naliases: [aa]\n---\nDe qué trata #a.\n",
    ),
    ("tags/hijo.md", "---\nparents: [a, otro]\n---\n"),
    (
        "tags/b2.md",
        "---\naliases:\n  - a2\nparents:\n  - A\n---\n",
    ),
    (
        "nota.md",
        "---\ntitle: x\ntags: [A, z]  # mío\n---\nHoy #a y #aa.\n",
    ),
    ("otra.md", "Sin tags.\n"),
];

#[test]
fn renaming_a_tag_with_page_and_undoing_it() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), RENAME);
    let before = contents(dir.path());
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    let plan = vault.tags().plan_rename("#a", "#b").unwrap();
    assert!(!plan.is_blocked());

    let applied = vault.apply(&plan.changes).unwrap();
    let after = contents(dir.path());
    let changed: Vec<(&str, &str)> = after
        .iter()
        .filter(|(file, text)| before.get(*file) != Some(text))
        .filter_map(|(file, text)| Some((file.as_str(), text.as_deref()?)))
        .collect();
    assert_eq!(
        changed,
        [
            (
                "nota.md",
                "---\ntitle: x\ntags: [b, z]  # mío\n---\nHoy #b y #aa.\n"
            ),
            (
                "tags/b.md",
                "---\ncolor: rojo\naliases: [aa]\n---\nDe qué trata #b.\n"
            ),
            (
                "tags/b2.md",
                "---\naliases:\n  - a2\nparents:\n  - b\n---\n"
            ),
            ("tags/hijo.md", "---\nparents: [b, otro]\n---\n"),
        ]
    );
    assert!(!after.contains_key("tags/a.md"));

    let report = vault.undo(&applied).unwrap();
    assert_eq!(contents(dir.path()), before);
    assert_eq!(report.changed, []);
    assert_eq!(report.restored.len(), 4);
}

#[test]
fn a_color_on_a_tag_without_page_creates_it_and_undo_removes_it() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), &[("nota.md", "#Rust y #rust\n")]);
    let before = contents(dir.path());
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    let color = PageChange::Color(Some(TagColor::Named("naranja".into())));
    let plan = vault.tags().plan_page_change("rust", &color).unwrap();

    let applied = vault.apply(&plan.changes).unwrap();
    assert_eq!(
        fs::read_to_string(dir.path().join("tags/Rust.md")).unwrap(),
        "---\ncolor: naranja\n---\n"
    );
    assert_eq!(
        applied.created().collect::<Vec<_>>(),
        [&path("tags/Rust.md")]
    );

    vault.undo(&applied).unwrap();
    assert_eq!(contents(dir.path()), before);
}

#[test]
fn a_file_that_changed_after_planning_stops_everything() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), RENAME);
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    let plan = vault.tags().plan_rename("#a", "#b").unwrap();
    let moved = "---\ntitle: nuevo\nparents: [a, otro]\n---\n";
    fs::write(dir.path().join("tags/hijo.md"), moved).unwrap();
    let before = contents(dir.path());
    let Err(ApplyError::Stale(stale)) = vault.apply(&plan.changes) else {
        panic!("se esperaba un archivo cambiado");
    };
    assert_eq!(stale.path, path("tags/hijo.md"));
    assert_eq!(contents(dir.path()), before);
}

#[test]
fn an_occupied_target_stops_everything() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), RENAME);
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    let plan = vault.tags().plan_rename("#a", "#b").unwrap();
    fs::write(dir.path().join("tags/b.md"), "de otro programa").unwrap();
    let before = contents(dir.path());
    let result = vault.apply(&plan.changes);
    assert!(
        matches!(
            &result,
            Err(ApplyError::Access(AccessError::Path(FileOpError::AlreadyExists(target))))
                if target == &path("tags/b.md")
        ),
        "{result:?}"
    );
    assert_eq!(contents(dir.path()), before);
}

#[test]
fn undo_leaves_what_changed_after_applying() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), RENAME);
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    let plan = vault.tags().plan_rename("#a", "#b").unwrap();
    let applied = vault.apply(&plan.changes).unwrap();
    let edited = "---\ntitle: x\ntags: [b, z]\n---\nOtra cosa.\n";
    fs::write(dir.path().join("nota.md"), edited).unwrap();

    let report = vault.undo(&applied).unwrap();
    assert_eq!(report.changed, [path("nota.md")]);
    assert_eq!(
        fs::read_to_string(dir.path().join("nota.md")).unwrap(),
        edited
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("tags/a.md")).unwrap(),
        RENAME[0].1
    );
}
