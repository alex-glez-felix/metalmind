use std::fs;

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
    let in_scope = focus.resolve(&vault.snapshot(), &vault.tags());
    let mut shown: Vec<String> = vault
        .snapshot()
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
    let focus = Focus::parse(text).unwrap().unwrap();
    assert_eq!(shown(&focus), expected);
}

#[test]
fn se_escribe_sin_espacios_de_mas() {
    let focus = Focus::parse("  proyectos/   #rust ").unwrap().unwrap();
    assert_eq!(focus.label(), "focus: proyectos/ #rust");
    assert_eq!(Focus::parse("   "), Ok(None));
}

#[test]
fn un_alcance_invalido_es_un_error() {
    assert_eq!(
        Focus::parse("#2024/10"),
        Err(ScopeError::InvalidTag("#2024/10".to_owned()))
    );
    assert_eq!(
        Focus::parse("a/ b/"),
        Err(ScopeError::TwoFolders("a/".to_owned(), "b/".to_owned()))
    );
}

#[test]
fn los_nodos_del_panel_son_el_mismo_alcance_que_escrito() {
    let tag = Focus::tag("Rust");
    assert_eq!(tag.label(), "focus: #Rust");
    assert!(tag.same_scope(&Focus::parse("#rust").unwrap().unwrap()));
    let folder = Focus::folder(VaultPath::new("proyectos/app").unwrap());
    assert_eq!(folder.label(), "focus: proyectos/app/");
    assert!(folder.same_scope(&Focus::parse("proyectos/app").unwrap().unwrap()));
    assert!(!folder.same_scope(&tag));
}

/// Si se renombra o se mueve su carpeta, el focus va con ella, y el texto
/// del indicador también.
#[test]
fn sigue_a_su_carpeta_si_se_renombra() {
    let mut focus = Focus::parse("proyectos/app #rust").unwrap().unwrap();
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
            &Focus::parse("#rust archivo/proyectos/app")
                .unwrap()
                .unwrap()
        )
    );
    let mut tags = Focus::parse("#rust").unwrap().unwrap();
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
    let focus = Focus::parse(text).unwrap().unwrap();
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
fn lo_que_se_ve_se_puede_volver_a_escribir(#[case] folder: &str) {
    let focus = Focus::folder(VaultPath::new(folder).unwrap());
    let written = focus.label().strip_prefix("focus: ").unwrap().to_owned();
    let parsed = Focus::parse(&written).unwrap().unwrap();
    assert!(parsed.same_scope(&focus), "{written:?}");
    assert_eq!(parsed.label(), focus.label());
}
