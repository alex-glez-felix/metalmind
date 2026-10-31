use pretty_assertions::assert_eq;
use rstest::rstest;

use super::*;

const CONFIG: &str = "\
# Mi config
theme: metalmind-claro   # el claro

editor:
  wrap: true # partir
  tabstop: 4

# vistas
views:
  gs: \"#gs -#archivado\"
  hoy: \"#hoy\"
";

#[rstest]
#[case::cambia_un_valor_y_deja_su_comentario(
    &["editor", "wrap"], "false",
    CONFIG.replace("wrap: true # partir", "wrap: false # partir")
)]
#[case::agrega_al_final_de_su_mapa(
    &["editor", "scrolloff"], "3",
    CONFIG.replace("tabstop: 4\n", "tabstop: 4\n  scrolloff: 3\n")
)]
#[case::agrega_un_mapa_que_falta_al_final(
    &["font", "size"], "15",
    format!("{CONFIG}font:\n  size: 15\n")
)]
#[case::una_clave_del_primer_nivel(
    &["theme"], "metalmind-oscuro",
    CONFIG.replace("theme: metalmind-claro", "theme: metalmind-oscuro")
)]
#[case::una_vista_nueva_con_comillas(
    &["views", "Para hoy"], &string("#hoy -#hecho"),
    CONFIG.replace("hoy: \"#hoy\"\n", "hoy: \"#hoy\"\n  Para hoy: \"#hoy -#hecho\"\n")
)]
fn set_cambia_solo_esa_clave(#[case] path: &[&str], #[case] value: &str, #[case] expected: String) {
    assert_eq!(set(CONFIG, path, value).unwrap(), expected);
}

#[rstest]
#[case::archivo_vacio("", "views:\n  gs: \"#gs\"\n")]
#[case::solo_comentarios("# nada\n", "# nada\nviews:\n  gs: \"#gs\"\n")]
#[case::sin_salto_al_final("theme: x", "theme: x\nviews:\n  gs: \"#gs\"\n")]
#[case::clave_vacia("views:\ntheme: x\n", "views:\n  gs: \"#gs\"\ntheme: x\n")]
#[case::crlf("theme: x\r\n", "theme: x\r\nviews:\r\n  gs: \"#gs\"\r\n")]
#[case::sangria_de_cuatro("views:\n    a: \"#a\"\n", "views:\n    a: \"#a\"\n    gs: \"#gs\"\n")]
#[case::reemplaza_un_valor_de_varias_lineas(
    "views:\n  gs:\n    - x\n    - y\ntheme: x\n",
    "views:\n  gs: \"#gs\"\ntheme: x\n"
)]
fn set_en_cada_forma_del_archivo(#[case] text: &str, #[case] expected: &str) {
    assert_eq!(
        set(text, &["views", "gs"], &string("#gs")).unwrap(),
        expected
    );
}

#[test]
fn lo_mismo_que_ya_dice_no_cambia_nada() {
    assert_eq!(set(CONFIG, &["editor", "tabstop"], "4").unwrap(), CONFIG);
}

#[rstest]
#[case::mapa_de_flujo("views: {gs: \"#gs\"}\n", "views")]
#[case::una_lista("views:\n  - a\n", "views")]
#[case::un_texto("views: nada\n", "views")]
#[case::documento_que_no_es_mapa("- a\n- b\n", "")]
fn lo_que_no_es_un_mapa_en_bloque_no_se_toca(#[case] text: &str, #[case] key: &str) {
    assert_eq!(
        set(text, &["views", "gs"], "x"),
        Err(EditError::Unsupported(key.to_owned()))
    );
}

#[test]
fn un_ancla_compartida_no_se_cambia_por_debajo() {
    let text = "base: &b\n  wrap: true\neditor: *b\n";
    assert_eq!(
        set(text, &["base", "wrap"], "false"),
        Err(EditError::Unsupported("base".to_owned()))
    );
}

#[test]
fn un_yaml_roto_es_error_con_su_posicion() {
    let Err(EditError::Unreadable(diagnostic)) = set("a: [\n", &["b"], "1") else {
        panic!("debió fallar");
    };
    assert!(diagnostic.location.is_some(), "{diagnostic:?}");
}

#[rstest]
#[case::una_de_varias(&["views", "gs"], CONFIG.replace("  gs: \"#gs -#archivado\"\n", ""))]
#[case::la_ultima_se_lleva_su_mapa(
    &["editor", "wrap"],
    CONFIG.replace("  wrap: true # partir\n", "")
)]
#[case::una_que_no_esta(&["views", "nada"], CONFIG.to_owned())]
#[case::bajo_un_mapa_que_no_esta(&["font", "size"], CONFIG.to_owned())]
fn remove_quita_solo_esa_clave(#[case] path: &[&str], #[case] expected: String) {
    assert_eq!(remove(CONFIG, path).unwrap(), expected);
}

#[test]
fn quitar_la_unica_quita_el_mapa_y_deja_lo_demas() {
    let text = "views:\n  gs: \"#gs\"\n# fin\ntheme: x\n";
    assert_eq!(remove(text, &["views", "gs"]).unwrap(), "# fin\ntheme: x\n");
}

#[test]
fn rename_cambia_la_clave_en_su_lugar() {
    let renamed = rename(CONFIG, &["views", "gs"], "Grupo de estudio").unwrap();
    assert_eq!(
        renamed,
        CONFIG.replace("  gs: \"#gs", "  Grupo de estudio: \"#gs")
    );
    let quoted = rename(CONFIG, &["views", "hoy"], "2024").unwrap();
    assert_eq!(quoted, CONFIG.replace("  hoy:", "  \"2024\":"));
}

#[test]
fn rename_no_pisa_ni_inventa() {
    assert_eq!(
        rename(CONFIG, &["views", "gs"], "hoy"),
        Err(EditError::Exists("views.hoy".to_owned()))
    );
    assert_eq!(
        rename(CONFIG, &["views", "nada"], "x"),
        Err(EditError::Missing("views.nada".to_owned()))
    );
    assert_eq!(rename(CONFIG, &["views", "gs"], "gs").unwrap(), CONFIG);
}

#[rstest]
#[case::comillas_simples("'a b': 1\n", "'a b'")]
#[case::comillas_dobles("\"a\\\"b\": 1\n", "\"a\\\"b\"")]
fn las_claves_entre_comillas_se_encuentran(#[case] text: &str, #[case] written: &str) {
    let path = [if written.starts_with('\'') {
        "a b"
    } else {
        "a\"b"
    }];
    assert_eq!(set(text, &path, "2").unwrap(), format!("{written}: 2\n"));
}

#[rstest]
#[case::numero("15")]
#[case::booleano("false")]
#[case::empieza_con_gato("#rust")]
#[case::comillas_y_barras("a \"b\" \\ c")]
#[case::clave_rara("true")]
fn lo_escrito_se_lee_como_era(#[case] text: &str) {
    let edited = set("", &[text], &string(text)).unwrap();
    let tree = read_tree(&edited).unwrap();
    assert_eq!(
        tree,
        Tree::Map(vec![(
            Tree::Str(text.to_owned()),
            Tree::Str(text.to_owned())
        )])
    );
}
