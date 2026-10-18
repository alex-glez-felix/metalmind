#![allow(
    clippy::unwrap_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

use std::fs;
use std::path::Path;

use jiff::Timestamp;
use jiff::tz::TimeZone;
use metalmind_template::{NewNote, Pick, Templates, draft};
use metalmind_vault::{Vault, VaultPath};
use pretty_assertions::assert_eq;

fn write(root: &Path, files: &[(&str, &str)]) {
    for (path, text) in files {
        let full = root.join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, text).unwrap();
    }
}

fn folder(path: &str) -> VaultPath {
    VaultPath::new(path).unwrap()
}

#[test]
fn a_tag_page_of_the_vault_picks_its_template() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        &[
            ("plantillas/reunion.md", "# {{title}}\n"),
            (
                "plantillas/trabajo/semanal.md",
                "semana {{date:YYYY-MM-DD}}\n",
            ),
            ("plantillas/logo.png", "no es plantilla"),
            (
                "tags/reunion.md",
                "---\ntemplate: reunion\n---\nReuniones.\n",
            ),
            ("tags/Semanal.md", "---\ntemplate: trabajo/semanal\n---\n"),
            ("tags/rust.md", "Sin frontmatter.\n"),
            ("tags/mi tag.md", "---\ntemplate: reunion\n---\n"),
            ("tags/viejo/x.md", "---\ntemplate: reunion\n---\n"),
            ("nota.md", "---\ntemplate: reunion\n---\n"),
        ],
    );
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    let index = vault.snapshot();

    let templates = Templates::load(vault.root(), &index, &folder("plantillas"));
    assert!(templates.errors.is_empty(), "{:?}", templates.errors);
    let names: Vec<_> = templates.value.names().collect();
    assert_eq!(names, ["reunion", "trabajo/semanal"]);

    let now = "2026-10-18T21:07:09Z"
        .parse::<Timestamp>()
        .unwrap()
        .to_zoned(TimeZone::posix("CST6").unwrap());
    let tags = vec!["semanal".to_owned()];
    let note = NewNote {
        title: "Lunes",
        tags: &tags,
        pick: Pick::ByTags,
        now: &now,
    };
    let draft = draft(&note, &templates.value, vault.tags().graph());
    assert_eq!(draft.problems, []);
    assert_eq!(draft.text, "---\ntags: [semanal]\n---\nsemana 2026-10-18\n");
}

#[test]
fn missing_folders_give_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    let index = vault.snapshot();
    let templates = Templates::load(vault.root(), &index, &folder("plantillas"));
    assert_eq!(templates.value, Templates::default());
}

#[test]
fn an_unreadable_template_is_reported_and_the_rest_load() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), &[("plantillas/buena.md", "ok")]);
    let bad = dir.path().join("plantillas/mala.md");
    fs::write(&bad, [0xff, 0xfe, 0x00]).unwrap();
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    let templates = Templates::load(vault.root(), &vault.snapshot(), &folder("plantillas"));
    let names: Vec<_> = templates.value.names().collect();
    assert_eq!(names, ["buena"]);
    assert_eq!(templates.errors.len(), 1, "{:?}", templates.errors);
}
