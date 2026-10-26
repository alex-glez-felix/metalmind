use std::fs;
use std::path::Path;

use jiff::Timestamp;
use jiff::tz::TimeZone;
use pretty_assertions::assert_eq;
use rstest::rstest;

use metalmind_template::NameError;

use super::*;
use crate::picker::LineEdit;

fn now() -> Zoned {
    "2026-10-26T15:30:00Z"
        .parse::<Timestamp>()
        .unwrap()
        .to_zoned(TimeZone::posix("CST6").unwrap())
}

const ENTREVISTA: &str =
    "---\ntags: [entrevista]\n---\n# {{title}}\n\n## Resumen\n\n## Preguntas\n";

/// Un vault con plantillas y páginas de tag, ya abierto.
fn vault(files: &[(&str, &str)]) -> (tempfile::TempDir, Vault) {
    let dir = tempfile::tempdir().unwrap();
    for (path, text) in files {
        let full = dir.path().join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, text).unwrap();
    }
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    (dir, vault)
}

fn standard() -> (tempfile::TempDir, Vault) {
    vault(&[
        ("plantillas/entrevista.md", ENTREVISTA),
        ("plantillas/trabajo/semanal.md", "# Semana del {{date}}\n"),
        ("plantillas/diario.md", "# {{date:dddd D [de] MMMM}}\n\n"),
        ("tags/reunion.md", "---\ntemplate: trabajo/semanal\n---\n"),
        ("tags/diario.md", "---\ntemplate: diario\n---\n"),
        ("tags/bitacora.md", "---\ntemplate: diario\n---\n"),
        ("tags/rust.md", "Sin plantilla.\n"),
        ("ya.md", "existe\n"),
    ])
}

fn templates(vault: &Vault) -> Templates {
    load_templates(vault, &vault.snapshot(), "plantillas").unwrap()
}

fn read(root: &Path, path: &str) -> String {
    fs::read_to_string(root.join(path)).unwrap()
}

#[rstest]
#[case::nada(None, None, None)]
#[case::en_blanco(Some("  "), None, None)]
#[case::plantilla(Some("entrevista"), Some("entrevista"), None)]
#[case::nombre_con_espacios(
    Some(" entrevista  Backend senior "),
    Some("entrevista"),
    Some("Backend senior")
)]
#[case::carpeta(
    Some("trabajo/semanal semanas/43"),
    Some("trabajo/semanal"),
    Some("semanas/43")
)]
fn pedidos(
    #[case] argument: Option<&str>,
    #[case] template: Option<&str>,
    #[case] name: Option<&str>,
) {
    assert_eq!(
        NoteRequest::parse(argument),
        NoteRequest {
            template: template.map(str::to_owned),
            name: name.map(str::to_owned),
        }
    );
}

#[test]
fn las_plantillas_listan_los_tags_que_las_piden() {
    let (_dir, vault) = standard();
    let items = template_items(&templates(&vault), vault.tags().graph());
    let rows: Vec<(&str, Option<&str>)> = items
        .iter()
        .map(|item| (item.name.as_str(), item.detail()))
        .collect();
    assert_eq!(
        rows,
        [
            ("diario", Some("#bitacora #diario")),
            ("entrevista", None),
            ("trabajo/semanal", Some("#reunion")),
        ]
    );
}

#[test]
fn una_carpeta_de_plantillas_invalida_es_error() {
    let (_dir, vault) = standard();
    assert!(load_templates(&vault, &vault.snapshot(), "../fuera").is_err());
    let none = load_templates(&vault, &vault.snapshot(), "no-existe").unwrap();
    assert_eq!(none.names().count(), 0);
}

#[test]
fn la_plantilla_conserva_sus_tags_y_el_cursor_va_a_la_primera_seccion() {
    let (dir, vault) = standard();
    let templates = templates(&vault);
    let origin = Origin::Template("entrevista".to_owned());
    let note = NotePath::parse("entrevistas/Backend senior").unwrap();
    let draft = origin.draft(&note.title, &templates, vault.tags().graph(), &now());
    let landing = create(&vault, &note, &draft).unwrap();
    let text = read(dir.path(), "entrevistas/Backend senior.md");
    assert_eq!(
        text,
        "---\ntags: [entrevista]\n---\n# Backend senior\n\n## Resumen\n\n## Preguntas\n"
    );
    assert_eq!(
        landing,
        Landing {
            path: note.path,
            cursor: Some(LinePosition { line: 6, column: 0 }),
            problems: Vec::new(),
        }
    );
}

#[test]
fn el_unico_tag_que_pide_la_plantilla_queda_puesto() {
    let (_dir, vault) = standard();
    let origin = Origin::Template("trabajo/semanal".to_owned());
    let draft = origin.draft("Semana", &templates(&vault), vault.tags().graph(), &now());
    assert_eq!(
        draft.text,
        "---\ntags: [reunion]\n---\n# Semana del 2026-10-26\n"
    );
}

#[test]
fn dos_tags_que_piden_la_misma_plantilla_no_ponen_ninguno() {
    let (_dir, vault) = standard();
    let origin = Origin::Template("diario".to_owned());
    let draft = origin.draft("hoy", &templates(&vault), vault.tags().graph(), &now());
    assert_eq!(draft.text, "# lunes 26 de octubre\n\n");
}

#[test]
fn una_nota_desde_un_tag_usa_la_plantilla_del_tag() {
    let (_dir, vault) = standard();
    let graph = vault.tags().graph().clone();
    let templates = templates(&vault);
    let from_tag =
        |tag: &str| Origin::Tag(tag.to_owned()).draft("Plan", &templates, &graph, &now());
    assert_eq!(
        from_tag("reunion").text,
        "---\ntags: [reunion]\n---\n# Semana del 2026-10-26\n"
    );
    let plain = from_tag("rust");
    assert_eq!(plain.text, "---\ntags: [rust]\n---\n");
    assert_eq!(plain.template, None);
}

#[test]
fn crear_nunca_pisa() {
    let (dir, vault) = standard();
    let note = NotePath::parse("ya").unwrap();
    let draft = Origin::Tag("rust".to_owned()).draft(
        "ya",
        &templates(&vault),
        vault.tags().graph(),
        &now(),
    );
    assert!(matches!(
        create(&vault, &note, &draft),
        Err(FileOpError::AlreadyExists(_))
    ));
    assert_eq!(read(dir.path(), "ya.md"), "existe\n");
}

#[test]
fn los_problemas_de_la_plantilla_llegan_con_la_nota() {
    let (_dir, vault) = vault(&[("plantillas/rota.md", "# {{autor}}\n")]);
    let draft = Origin::Template("rota".to_owned()).draft(
        "x",
        &templates(&vault),
        vault.tags().graph(),
        &now(),
    );
    let landing = create(&vault, &NotePath::parse("x").unwrap(), &draft).unwrap();
    assert_eq!(
        landing.problems,
        ["en la plantilla: `{{autor}}` no es una variable de plantilla"]
    );
}

fn daily(template: Option<&str>) -> Daily {
    Daily {
        enabled: true,
        template: template.map(str::to_owned),
        ..Daily::default()
    }
}

#[test]
fn la_nota_del_dia_se_crea_una_vez_y_despues_se_abre() {
    let (dir, vault) = standard();
    let templates = templates(&vault);
    let first = open_daily(&vault, &daily(Some("diario")), &templates, &now()).unwrap();
    assert_eq!(first.path.as_str(), "diario/2026-10-26.md");
    assert_eq!(first.cursor, Some(LinePosition { line: 1, column: 0 }));
    let path = "diario/2026-10-26.md";
    assert_eq!(read(dir.path(), path), "# lunes 26 de octubre\n\n");
    fs::write(dir.path().join(path), "escrita\n").unwrap();
    let again = open_daily(&vault, &daily(Some("diario")), &templates, &now()).unwrap();
    assert_eq!(again.cursor, None);
    assert_eq!(read(dir.path(), path), "escrita\n");
}

#[test]
fn la_nota_del_dia_esta_apagada_por_defecto() {
    let (_dir, vault) = standard();
    let error = open_daily(&vault, &Daily::default(), &templates(&vault), &now()).unwrap_err();
    assert!(matches!(error, DailyOpenError::Disabled), "{error}");
}

#[test]
fn un_formato_de_dia_invalido_es_error() {
    let (_dir, vault) = standard();
    let settings = Daily {
        format: String::new(),
        ..daily(None)
    };
    let error = open_daily(&vault, &settings, &templates(&vault), &now()).unwrap_err();
    assert!(matches!(error, DailyOpenError::Settings(_)), "{error}");
}

#[rstest]
#[case("", 0, 0, 0)]
#[case("a\nbc\n", 3, 1, 1)]
#[case("ñ\n\n", 3, 1, 0)]
#[case("a", 9, 0, 1)]
fn posiciones(#[case] text: &str, #[case] at: usize, #[case] line: usize, #[case] column: usize) {
    assert_eq!(line_position(text, at), LinePosition { line, column });
}

fn question(vault: &Vault) -> NameQuestion {
    NameQuestion::new(
        Origin::Template("entrevista".to_owned()),
        templates(vault),
        vault.snapshot(),
    )
}

fn typed(question: &mut NameQuestion, text: &str) {
    question.apply(LineEdit::End);
    question.apply(LineEdit::DeleteToStart);
    question.apply(LineEdit::Insert(text));
}

#[test]
fn la_pregunta_del_nombre_dice_que_falta() {
    let (_dir, vault) = standard();
    let mut question = question(&vault);
    let answer =
        |question: &NameQuestion| question.answer().map(|note| note.path.as_str().to_owned());
    assert_eq!(answer(&question), Err(NameProblem::Name(NameError::Empty)));
    typed(&mut question, "ya");
    assert_eq!(
        answer(&question),
        Err(NameProblem::Exists(VaultPath::new("ya.md").unwrap()))
    );
    typed(&mut question, "../fuera");
    assert!(matches!(
        answer(&question),
        Err(NameProblem::Name(NameError::Path(_)))
    ));
    typed(&mut question, "entrevistas/Backend");
    assert_eq!(answer(&question), Ok("entrevistas/Backend.md".to_owned()));
}

#[test]
fn un_fallo_al_crear_dura_hasta_cambiar_el_nombre() {
    let (_dir, vault) = standard();
    let mut question = question(&vault);
    typed(&mut question, "nueva");
    question.fail("sin permiso".to_owned());
    assert_eq!(
        question.answer(),
        Err(NameProblem::Failed("sin permiso".to_owned()))
    );
    question.apply(LineEdit::Left);
    assert!(
        question.answer().is_err(),
        "mover el cursor no cambia el nombre"
    );
    question.apply(LineEdit::Insert("s"));
    assert!(question.answer().is_ok());
}
