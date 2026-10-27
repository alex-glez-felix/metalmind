//! Una nota nueva en focus (05 §3): nace dentro de su carpeta y con sus
//! tags, y con la plantilla que pidan, para no desaparecer del focus.

use std::fs;

use jiff::Timestamp;
use jiff::tz::TimeZone;
use metalmind_template::NameError;
use pretty_assertions::assert_eq;

use super::*;
use crate::focus::{Focus, FocusedNote};
use crate::picker::LineEdit;

fn now() -> Zoned {
    "2026-10-26T15:30:00Z"
        .parse::<Timestamp>()
        .unwrap()
        .to_zoned(TimeZone::posix("CST6").unwrap())
}

fn vault() -> (tempfile::TempDir, Vault) {
    let dir = tempfile::tempdir().unwrap();
    for (path, text) in [
        ("plantillas/semanal.md", "# Semana del {{date}}\n"),
        ("tags/reunion.md", "---\ntemplate: semanal\n---\n"),
        ("tags/rust.md", "Sin plantilla.\n"),
        ("proyectos/a.md", "#Rust #reunion\n"),
    ] {
        let full = dir.path().join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, text).unwrap();
    }
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    (dir, vault)
}

fn focused(vault: &Vault, focus: &str) -> FocusedNote {
    Focus::parse(focus)
        .unwrap()
        .unwrap()
        .new_note(&vault.tags())
}

fn blank(vault: &Vault, focus_tags: &[String]) -> String {
    let templates = load_templates(vault, &vault.snapshot(), "plantillas").unwrap();
    Origin::Blank
        .draft("Plan", focus_tags, &templates, vault.tags().graph(), &now())
        .text
}

#[test]
fn sin_focus_la_nota_del_panel_nace_vacia() {
    let (_dir, vault) = vault();
    assert_eq!(blank(&vault, &[]), "");
}

/// Con la forma establecida de cada tag incluido, no los excluidos, y la
/// plantilla que pide la página de uno.
#[test]
fn en_focus_de_tags_nace_con_ellos_y_su_plantilla() {
    let (_dir, vault) = vault();
    let note = focused(&vault, "#rust #REUNION -#archivado");
    assert_eq!(note.tags, ["Rust", "reunion"]);
    assert_eq!(
        blank(&vault, &note.tags),
        "---\ntags: [Rust, reunion]\n---\n# Semana del 2026-10-26\n"
    );
}

#[test]
fn un_tag_del_focus_que_ya_trae_la_nota_no_se_repite() {
    let (_dir, vault) = vault();
    let templates = load_templates(&vault, &vault.snapshot(), "plantillas").unwrap();
    let focus_tags = ["Rust".to_owned(), "ui".to_owned()];
    let draft = Origin::Tag("rust".to_owned()).draft(
        "Plan",
        &focus_tags,
        &templates,
        vault.tags().graph(),
        &now(),
    );
    assert_eq!(draft.text, "---\ntags: [rust, ui]\n---\n");
}

#[test]
fn en_focus_de_carpeta_el_nombre_va_dentro_de_ella() {
    let (_dir, vault) = vault();
    let templates = load_templates(&vault, &vault.snapshot(), "plantillas").unwrap();
    let mut question = NameQuestion::new(Origin::Blank, templates, vault.snapshot())
        .in_focus(focused(&vault, "proyectos/"));
    let answer =
        |question: &NameQuestion| question.answer().map(|note| note.path.as_str().to_owned());
    assert_eq!(answer(&question), Err(NameProblem::Name(NameError::Empty)));
    question.apply(LineEdit::Insert("  web/Plan "));
    assert_eq!(answer(&question), Ok("proyectos/web/Plan.md".to_owned()));
    question.apply(LineEdit::DeleteToStart);
    question.apply(LineEdit::Insert("a"));
    assert_eq!(
        answer(&question),
        Err(NameProblem::Exists(
            VaultPath::new("proyectos/a.md").unwrap()
        ))
    );
}

fn settings(templates_folder: &str) -> NoteSettings {
    NoteSettings {
        templates_folder: templates_folder.to_owned(),
        ..NoteSettings::default()
    }
}

fn in_panel(
    vault: &Vault,
    folder: &str,
    path: &str,
    focus_tags: &[String],
) -> Result<Landing, PanelNoteError> {
    let path = VaultPath::new(path).unwrap();
    create_in_panel(
        vault,
        &settings(folder),
        &path,
        focus_tags,
        vault.tags().graph(),
        &now(),
    )
}

/// `a` en el panel, en focus de tags, crea la nota con ellos y su plantilla.
#[test]
fn en_el_panel_la_nota_nace_con_los_tags_del_focus() {
    let (dir, vault) = vault();
    let landing = in_panel(
        &vault,
        "plantillas",
        "proyectos/Plan.md",
        &["reunion".to_owned()],
    )
    .unwrap();
    assert_eq!(landing.path.as_str(), "proyectos/Plan.md");
    assert_eq!(
        fs::read_to_string(dir.path().join("proyectos/Plan.md")).unwrap(),
        "---\ntags: [reunion]\n---\n# Semana del 2026-10-26\n"
    );
}

/// Sin tags del focus no se leen las plantillas: una carpeta de plantillas
/// que no vale no impide crear. Con tags, sí, y no se crea nada.
#[test]
fn las_plantillas_se_leen_solo_si_hay_tags_del_focus() {
    let (dir, vault) = vault();
    in_panel(&vault, "/", "nueva.md", &[]).unwrap();
    assert_eq!(fs::read_to_string(dir.path().join("nueva.md")).unwrap(), "");
    assert!(matches!(
        in_panel(&vault, "/", "otra.md", &["reunion".to_owned()]),
        Err(PanelNoteError::Folder(_))
    ));
    assert!(!dir.path().join("otra.md").exists());
}
