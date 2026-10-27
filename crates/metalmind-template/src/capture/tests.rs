use std::fs;

use jiff::Timestamp;
use jiff::tz::TimeZone;
use metalmind_vault::NewNote as Captures;
use pretty_assertions::assert_eq;
use rstest::rstest;

use super::*;

fn now() -> Zoned {
    "2026-10-26T15:30:00Z"
        .parse::<Timestamp>()
        .unwrap()
        .to_zoned(TimeZone::posix("CST6").unwrap())
}

const FILES: &[(&str, &str)] = &[
    (
        "plantillas/entrevista.md",
        "---\ntags: [entrevista]\n---\n# {{title}}\n\n## Resumen\n\n## Preguntas\n",
    ),
    ("plantillas/semanal.md", "# Semana del {{date}}\n"),
    ("plantillas/diario.md", "# {{date}}\n"),
    ("tags/entrevista.md", "---\ntemplate: entrevista\n---\n"),
    (
        "tags/reunion.md",
        "---\ntemplate: semanal\naliases: [junta]\n---\n",
    ),
    ("tags/bitacora.md", "---\ntemplate: diario\n---\n"),
    ("tags/rust.md", "Sin plantilla.\n"),
    ("tags/vieja.md", "---\ntemplate: borrada\n---\n"),
];

fn vault() -> (tempfile::TempDir, VaultRoot) {
    let dir = tempfile::tempdir().unwrap();
    for (path, text) in FILES {
        let full = dir.path().join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, text).unwrap();
    }
    let root = VaultRoot::new(dir.path()).unwrap();
    (dir, root)
}

fn tags_folder() -> VaultPath {
    VaultPath::new("tags").unwrap()
}

fn add(
    root: &VaultRoot,
    text: &str,
    title: Option<&str>,
    tags: &[&str],
    pick: Pick<'_>,
) -> Result<(FromTemplate, String), AddError> {
    let note = Captures {
        text: text.to_owned(),
        title: title.map(str::to_owned),
        tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
    };
    let folders = Folders {
        templates: "plantillas/",
        tags: &tags_folder(),
    };
    let added = capture(root, &note, pick, folders, &now())?;
    let written = fs::read_to_string(root.path().join(added.captured.path.as_str())).unwrap();
    Ok((added, written))
}

/// La plantilla elegida, con el tag cuya página la pide, `{{title}}` del
/// título y lo capturado bajo la primera sección vacía.
#[test]
fn con_plantilla_el_texto_va_donde_se_empieza_a_escribir() {
    let (_dir, root) = vault();
    let (added, written) = add(
        &root,
        "Buena comunicación.\n\n",
        Some("Backend senior"),
        &["rust"],
        Pick::Named("entrevista"),
    )
    .unwrap();
    assert_eq!(added.captured.path.as_str(), "backend-senior.md");
    assert_eq!(added.captured.tags, ["rust", "entrevista"]);
    assert_eq!(added.template.as_deref(), Some("entrevista"));
    assert_eq!(added.problems, Vec::<String>::new());
    assert_eq!(
        written,
        "---\ntags: [entrevista, rust]\n---\n# Backend senior\n\n## Resumen\nBuena comunicación.\n\n## Preguntas\n"
    );
}

/// Sin título, `{{title}}` es el nombre del archivo, y sin texto queda la
/// plantilla sola.
#[test]
fn sin_titulo_el_nombre_llena_title() {
    let (_dir, root) = vault();
    let (added, written) = add(&root, "", None, &[], Pick::Named("entrevista")).unwrap();
    assert_eq!(added.captured.path.as_str(), "2026-10-26-093000.md");
    assert!(written.contains("# 2026-10-26-093000\n"), "{written}");
}

/// `-t` con un tag (o un alias) cuya página pide plantilla la usa (06 §11).
#[rstest]
#[case::por_su_nombre("reunion")]
#[case::por_un_alias("#junta")]
fn un_tag_que_pide_plantilla_la_usa(#[case] tag: &str) {
    let (_dir, root) = vault();
    let (added, written) = add(&root, "acuerdos", None, &[tag], Pick::ByTags).unwrap();
    assert_eq!(added.template.as_deref(), Some("semanal"));
    let tag = tag.trim_start_matches('#');
    assert_eq!(
        written,
        format!("---\ntags: [{tag}]\n---\n# Semana del 2026-10-26\nacuerdos\n")
    );
}

/// Sin plantilla que aplique es la captura de siempre, con sus tags entre
/// comillas; si dos tags piden plantillas distintas, también, con el aviso.
#[rstest]
#[case::ningun_tag_pide(&["rust"], Pick::ByTags, 0)]
#[case::sin_plantilla_a_proposito(&["reunion"], Pick::Blank, 0)]
#[case::piden_distintas(&["reunion", "bitacora"], Pick::ByTags, 1)]
fn sin_plantilla_es_la_captura_de_siempre(
    #[case] tags: &[&str],
    #[case] pick: Pick<'_>,
    #[case] problems: usize,
) {
    let (_dir, root) = vault();
    let (added, written) = add(&root, "idea", None, tags, pick).unwrap();
    assert_eq!(added.template, None);
    assert_eq!(added.problems.len(), problems, "{:?}", added.problems);
    let quoted: Vec<String> = tags.iter().map(|tag| format!("\"{tag}\"")).collect();
    assert_eq!(
        written,
        format!("---\ntags: [{}]\n---\nidea\n", quoted.join(", "))
    );
}

#[test]
fn una_plantilla_que_no_existe_o_un_frontmatter_no_crean_nada() {
    let (dir, root) = vault();
    assert!(matches!(
        add(&root, "x", None, &[], Pick::Named("nada")),
        Err(AddError::MissingTemplate { .. })
    ));
    assert!(matches!(
        add(
            &root,
            "---\na: 1\n---\nx",
            None,
            &[],
            Pick::Named("semanal")
        ),
        Err(AddError::FrontmatterWithTemplate)
    ));
    assert!(!dir.path().join("x.md").exists());
}

#[rstest]
#[case::en_una_linea_en_blanco("# A\n\n## B\n", 4, "texto\n", "# A\ntexto\n\n## B\n")]
#[case::al_final_de_un_heading("# A\n## B\n", 3, "texto", "# A\ntexto\n## B\n")]
#[case::al_final("# A\n", 3, "texto\n\n", "# A\ntexto\n")]
#[case::sin_texto("# A\n\n", 4, "", "# A\n")]
#[case::plantilla_vacia("", 0, "texto", "texto\n")]
#[case::crlf_en_una_linea_en_blanco(
    "# A\r\n\r\n## B\r\n",
    5,
    "texto\n",
    "# A\r\ntexto\r\n\r\n## B\r\n"
)]
#[case::crlf_al_final_de_un_heading("# A\r\n## B\r\n", 3, "texto", "# A\r\ntexto\r\n## B\r\n")]
fn el_texto_va_en_su_linea(
    #[case] text: &str,
    #[case] cursor: usize,
    #[case] captured: &str,
    #[case] expected: &str,
) {
    assert_eq!(with_text(text, cursor, captured), expected);
}

/// La página de un tag pide una plantilla que ya no existe: es la captura
/// de siempre, con su título, y el aviso dice cuál falta y quién la pide.
#[test]
fn un_tag_que_pide_una_plantilla_borrada_es_la_captura_de_siempre() {
    let (_dir, root) = vault();
    let (added, written) = add(&root, "texto", Some("Plan"), &["vieja"], Pick::ByTags).unwrap();
    assert_eq!(added.template, None);
    assert_eq!(written, "---\ntags: [\"vieja\"]\n---\n# Plan\n\ntexto\n");
    assert_eq!(added.problems.len(), 1, "{:?}", added.problems);
    assert!(
        added.problems[0].contains("borrada"),
        "{:?}",
        added.problems
    );
}

/// La carpeta de plantillas de la config se lee solo si puede hacer falta:
/// con un tag que no pide plantilla, tampoco.
#[test]
fn una_carpeta_de_plantillas_que_no_vale_con_un_tag_sin_plantilla() {
    let (_dir, root) = vault();
    let note = Captures {
        text: "idea".to_owned(),
        tags: vec!["rust".to_owned()],
        ..Captures::default()
    };
    let broken = Folders {
        templates: "/",
        tags: &tags_folder(),
    };
    let added = capture(&root, &note, Pick::ByTags, broken, &now()).unwrap();
    assert_eq!(added.captured.path.as_str(), "idea.md");
}

/// La carpeta de plantillas de la config se lee solo si puede hacer falta.
#[test]
fn una_carpeta_de_plantillas_que_no_vale_solo_importa_con_plantilla() {
    let (_dir, root) = vault();
    let note = Captures {
        text: "idea".to_owned(),
        tags: vec!["reunion".to_owned()],
        ..Captures::default()
    };
    let broken = Folders {
        templates: "/",
        tags: &tags_folder(),
    };
    assert!(matches!(
        capture(&root, &note, Pick::ByTags, broken, &now()),
        Err(AddError::TemplatesFolder(_))
    ));
    let added = capture(&root, &note, Pick::Blank, broken, &now()).unwrap();
    assert_eq!(added.captured.path.as_str(), "idea.md");
}

/// Una página de tag que no se puede leer no pide plantilla, y se avisa.
#[test]
fn una_pagina_de_tag_ilegible_se_avisa() {
    use std::os::unix::fs::PermissionsExt as _;
    let (dir, root) = vault();
    let page = dir.path().join("tags/reunion.md");
    fs::set_permissions(&page, fs::Permissions::from_mode(0o000)).unwrap();
    let added = add(&root, "acuerdos", None, &["reunion"], Pick::ByTags);
    fs::set_permissions(&page, fs::Permissions::from_mode(0o644)).unwrap();
    let (added, written) = added.unwrap();
    assert_eq!(added.template, None);
    assert_eq!(written, "---\ntags: [\"reunion\"]\n---\nacuerdos\n");
    assert_eq!(added.problems.len(), 1, "{:?}", added.problems);
    assert!(
        added.problems[0].contains("reunion.md"),
        "{:?}",
        added.problems
    );
}

/// Una plantilla que no se puede leer se avisa una vez, y solo si se
/// buscaba una plantilla: con un tag que no pide ninguna, no.
#[test]
fn una_plantilla_ilegible_se_avisa_una_vez_y_solo_si_hace_falta() {
    use std::os::unix::fs::PermissionsExt as _;
    let (dir, root) = vault();
    let template = dir.path().join("plantillas/diario.md");
    fs::set_permissions(&template, fs::Permissions::from_mode(0o000)).unwrap();
    let plain = add(&root, "idea", None, &["rust"], Pick::ByTags);
    let semanal = add(&root, "acuerdos", None, &["reunion"], Pick::ByTags);
    fs::set_permissions(&template, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(plain.unwrap().0.problems, Vec::<String>::new());
    let (added, _) = semanal.unwrap();
    assert_eq!(added.template.as_deref(), Some("semanal"));
    assert_eq!(added.problems.len(), 1, "{:?}", added.problems);
    assert!(
        added.problems[0].contains("diario.md"),
        "{:?}",
        added.problems
    );
}
