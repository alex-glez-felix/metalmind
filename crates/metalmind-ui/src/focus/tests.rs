use std::fs;

use metalmind_config::SavedView;
use metalmind_vault::Vault;
use pretty_assertions::assert_eq;

use super::*;

const FILES: &[(&str, &str)] = &[
    ("proyectos/app/parser.md", "#rust"),
    ("proyectos/app/logo.png", ""),
    ("proyectos/viejo.md", "#rust #archivado"),
    ("proyectos/dibujos/boceto.png", ""),
    ("notas/gpui.md", "#gpui"),
    ("tags/gpui.md", "---\nparents: [rust]\n---\n"),
    ("suelta.md", "nada"),
];

fn vault() -> (tempfile::TempDir, Vault) {
    let dir = tempfile::tempdir().unwrap();
    for (name, text) in FILES {
        let path = dir.path().join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    (dir, vault)
}

fn shown(focus: &Focus) -> Vec<String> {
    let (_dir, vault) = vault();
    let snapshot = vault.snapshot();
    let in_scope = focus.resolve(&snapshot.files, &snapshot.tags);
    let mut shown: Vec<String> = vault
        .snapshot()
        .files
        .iter()
        .filter(|entry| in_scope.shows(entry.path))
        .map(|entry| entry.path.to_string())
        .collect();
    shown.sort();
    shown
}

/// Lo que se ve son los archivos del alcance y las carpetas que los tienen;
/// una carpeta sin nada del alcance se esconde (05 §3).
#[rstest::rstest]
#[case::carpeta(
    "proyectos",
    &["proyectos", "proyectos/app", "proyectos/app/logo.png", "proyectos/app/parser.md", "proyectos/dibujos", "proyectos/dibujos/boceto.png", "proyectos/viejo.md"],
)]
#[case::tag_con_descendientes("#rust", &["notas", "notas/gpui.md", "proyectos", "proyectos/app", "proyectos/app/parser.md", "proyectos/viejo.md"])]
#[case::carpeta_y_tags("proyectos/ #rust -#archivado", &["proyectos", "proyectos/app", "proyectos/app/parser.md"])]
#[case::buzon("-#", &["suelta.md"])]
#[case::paginas_de_tag("tags/", &["tags", "tags/gpui.md"])]
fn deja_ver_lo_del_alcance(#[case] text: &str, #[case] expected: &[&str]) {
    let focus = Focus::parse(text, &[]).unwrap().unwrap();
    assert_eq!(shown(&focus), expected);
}

#[test]
fn se_escribe_sin_espacios_de_mas() {
    let focus = Focus::parse("  proyectos/   #rust ", &[]).unwrap().unwrap();
    assert_eq!(focus.label(), "focus: proyectos/ #rust");
    assert_eq!(Focus::parse("   ", &[]), Ok(None));
}

#[test]
fn un_alcance_invalido_es_un_error() {
    assert_eq!(
        Focus::parse("#2024/10", &[]),
        Err(FocusError::Scope(ScopeError::InvalidTag(
            "#2024/10".to_owned()
        )))
    );
    assert_eq!(
        Focus::parse("a/ b/", &[]),
        Err(FocusError::Scope(ScopeError::TwoFolders(
            "a/".to_owned(),
            "b/".to_owned()
        )))
    );
}

#[test]
fn los_nodos_del_panel_son_el_mismo_alcance_que_escrito() {
    let tag = Focus::tag("Rust");
    assert_eq!(tag.label(), "focus: #Rust");
    assert!(tag.same_scope(&Focus::parse("#rust", &[]).unwrap().unwrap()));
    let folder = Focus::folder(VaultPath::new("proyectos/app").unwrap());
    assert_eq!(folder.label(), "focus: proyectos/app/");
    assert!(folder.same_scope(&Focus::parse("proyectos/app", &[]).unwrap().unwrap()));
    assert!(!folder.same_scope(&tag));
}

/// Si se renombra o se mueve su carpeta, el focus va con ella, y el texto
/// del indicador también.
#[test]
fn sigue_a_su_carpeta_si_se_renombra() {
    let mut focus = Focus::parse("proyectos/app #rust", &[]).unwrap().unwrap();
    let renamed = |from: &str, to: &str| metalmind_vault::VaultChange::Renamed {
        from: VaultPath::new(from).unwrap(),
        to: VaultPath::new(to).unwrap(),
        kind: metalmind_vault::EntryKind::Folder,
    };
    assert!(!focus.follow(&[renamed("otra", "nueva")]));
    assert!(focus.follow(&[renamed("proyectos", "archivo/proyectos")]));
    assert_eq!(focus.label(), "focus: archivo/proyectos/app/ #rust");
    assert!(
        focus.same_scope(
            &Focus::parse("#rust archivo/proyectos/app", &[])
                .unwrap()
                .unwrap()
        )
    );
    let mut tags = Focus::parse("#rust", &[]).unwrap().unwrap();
    assert!(!tags.follow(&[renamed("proyectos", "otros")]));
}

/// Una carpeta con espacios se escribe como en vim, `\ `, o entre comillas,
/// y el indicador la muestra con `\ ` (#157).
#[rstest::rstest]
#[case::escapada(r"Mis\ notas/ #rust", r"focus: Mis\ notas/ #rust")]
#[case::entre_comillas(r#""Mis notas" #rust"#, r"focus: Mis\ notas/ #rust")]
#[case::comillas_en_medio(r#"Mis" "notas/ #rust"#, r"focus: Mis\ notas/ #rust")]
#[case::parece_un_tag(r"\#ideas", r"focus: \#ideas/")]
#[case::parece_el_buzon(r#""-#" -#"#, r"focus: \-#/ -#")]
fn una_carpeta_con_espacios_se_escribe_escapada(#[case] text: &str, #[case] label: &str) {
    let focus = Focus::parse(text, &[]).unwrap().unwrap();
    assert_eq!(focus.label(), label);
}

/// Lo que muestra el indicador, escrito en `:focus`, es el mismo alcance.
#[rstest::rstest]
#[case("Mis notas")]
#[case("a/b c/d e")]
#[case(r"con\barra")]
#[case("con \"comillas\"")]
#[case("#ideas")]
#[case("-#ideas")]
#[case("-x")]
#[case("@ideas")]
fn lo_que_se_ve_se_puede_volver_a_escribir(#[case] folder: &str) {
    let focus = Focus::folder(VaultPath::new(folder).unwrap());
    let written = focus.label().strip_prefix("focus: ").unwrap().to_owned();
    let parsed = Focus::parse(&written, &[]).unwrap().unwrap();
    assert!(parsed.same_scope(&focus), "{written:?}");
    assert_eq!(parsed.label(), focus.label());
}

fn saved(name: &str, query: &str) -> SavedView {
    SavedView {
        name: name.to_owned(),
        query: query.to_owned(),
    }
}

fn views() -> Vec<SavedView> {
    vec![
        saved("gs", "#rust -#archivado"),
        saved("Para hoy", "#gpui"),
        saved("con texto", "#rust parser"),
    ]
}

/// `@nombre` es la vista guardada: su consulta, con lo demás que se escriba
/// (06 §10). El nombre no distingue mayúsculas, y el indicador lo escribe
/// como está guardado.
#[rstest::rstest]
#[case::una_vista("@gs", "focus: @gs", "#rust -#archivado")]
#[case::con_carpeta_y_tags(
    "proyectos/ @GS #nota",
    "focus: proyectos/ @gs #nota",
    "proyectos/ #rust -#archivado #nota"
)]
#[case::con_espacios(r"@para\ hoy", r"focus: @Para\ hoy", "#gpui")]
#[case::entre_comillas(r#"@"Para hoy""#, r"focus: @Para\ hoy", "#gpui")]
fn una_vista_suma_su_consulta(#[case] text: &str, #[case] label: &str, #[case] same_as: &str) {
    let focus = Focus::parse(text, &views()).unwrap().unwrap();
    assert_eq!(focus.label(), label);
    assert!(focus.same_scope(&Focus::parse(same_as, &[]).unwrap().unwrap()));
    let again = label.strip_prefix("focus: ").unwrap();
    assert_eq!(Focus::parse(again, &views()).unwrap().unwrap(), focus);
}

#[test]
fn una_vista_que_no_esta_o_con_texto_es_un_error() {
    assert_eq!(
        Focus::parse("@nada", &views()),
        Err(FocusError::UnknownView("nada".to_owned()))
    );
    assert_eq!(
        Focus::parse("@", &views()),
        Err(FocusError::UnknownView(String::new()))
    );
    let error = FocusError::NotTags {
        name: "con texto".to_owned(),
        word: "parser".to_owned(),
    };
    assert_eq!(Focus::parse(r"@con\ texto", &views()), Err(error.clone()));
    assert_eq!(Focus::view(&views()[2]), Err(error));
}

/// Escapado, `\@ideas` es la carpeta `@ideas`.
#[test]
fn una_arroba_escapada_es_una_carpeta() {
    let focus = Focus::parse(r"\@ideas", &views()).unwrap().unwrap();
    assert_eq!(
        focus.scope().folder,
        Some(VaultPath::new("@ideas").unwrap())
    );
    assert_eq!(focus.label(), r"focus: \@ideas/");
}

/// `F` sobre una vista del panel es lo mismo que `:focus @nombre`, y en
/// focus de una vista solo queda lo de su consulta.
#[test]
fn el_nodo_de_una_vista_es_su_consulta() {
    let focus = Focus::view(&views()[0]).unwrap();
    assert_eq!(focus.label(), "focus: @gs");
    assert_eq!(focus, Focus::parse("@gs", &views()).unwrap().unwrap());
    assert_eq!(
        shown(&focus),
        [
            "notas",
            "notas/gpui.md",
            "proyectos",
            "proyectos/app",
            "proyectos/app/parser.md"
        ]
    );
}

/// El focus de una vista la sigue si se renombra o cambia su consulta; si
/// se borra, queda con los tags que tenía.
#[test]
fn sigue_a_su_vista() {
    let mut focus = Focus::parse("proyectos/ @gs #nota", &views())
        .unwrap()
        .unwrap();
    assert!(!focus.follow_views(&views(), None));
    let renamed = [saved("estudio", "#rust -#archivado")];
    assert!(focus.follow_views(&renamed, Some(("gs", "estudio"))));
    assert_eq!(focus.label(), "focus: proyectos/ @estudio #nota");
    let changed = [saved("estudio", "#lenguaje")];
    assert!(focus.follow_views(&changed, None));
    assert!(
        focus.same_scope(
            &Focus::parse("proyectos/ #lenguaje #nota", &[])
                .unwrap()
                .unwrap()
        )
    );
    assert!(focus.follow_views(&[], None));
    assert_eq!(focus.label(), "focus: proyectos/ #lenguaje #nota");
    assert!(
        focus.same_scope(
            &Focus::parse("proyectos/ #lenguaje #nota", &[])
                .unwrap()
                .unwrap()
        )
    );
}

/// Una vista que deja de ser solo de tags no cambia el alcance: queda como
/// los tags que tenía.
#[test]
fn una_vista_que_se_rompe_deja_sus_tags() {
    let mut focus = Focus::parse("@gs", &views()).unwrap().unwrap();
    assert!(focus.follow_views(&[saved("gs", "#rust idea")], None));
    assert_eq!(focus.label(), "focus: #rust -#archivado");
}
