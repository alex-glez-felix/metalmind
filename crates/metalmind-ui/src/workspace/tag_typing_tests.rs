//! El tag que se está escribiendo y aceptar su autocompletado (05 §3).

use metalmind_syntax::TagPlace;
use metalmind_vim::Mode;
use pretty_assertions::assert_eq;

use super::tag_typing::TypedTag;
use crate::workspace::test_session::Session;

fn typed(session: &Session) -> Option<TypedTag> {
    session.workspace.typed_tag(session.pane()).unwrap()
}

fn inline(typed: &str) -> TypedTag {
    TypedTag {
        typed: typed.to_owned(),
        place: TagPlace::Inline,
    }
}

/// Lo que va del nombre, con el cursor al final; solo en Insert.
#[test]
fn en_el_cuerpo_con_el_cursor_al_final_del_nombre() {
    let mut session = Session::new("hola\n");
    session.keys("A #r");
    assert_eq!(typed(&session), Some(inline("r")));
    session.keys("u");
    assert_eq!(typed(&session), Some(inline("ru")));
    session.keys("⎋");
    assert_eq!(typed(&session), None, "en Normal no se completa");
}

/// `#` solo todavía no es un tag, y `# ` es un heading.
#[rstest::rstest]
#[case::solo_el_numeral("A #")]
#[case::heading("o# r")]
#[case::tras_el_nombre("A #ru ")]
#[case::pegado_a_una_palabra("A x#ru")]
fn lo_que_no_es_un_tag(#[case] keys: &str) {
    let mut session = Session::new("hola\n");
    session.keys(keys);
    assert_eq!(typed(&session), None);
}

#[test]
fn en_codigo_no() {
    let mut session = Session::new("```\n\n```\n");
    session.keys("ji#ru");
    assert_eq!(typed(&session), None);
    let mut session = Session::new("hola\n");
    session.keys("A `#ru");
    assert_eq!(typed(&session), None);
}

/// En `tags:` del frontmatter, con el lugar para escribir un escalar de
/// YAML; entre comillas no, porque reemplazarlo tecleando las rompería.
#[test]
fn en_tags_del_frontmatter() {
    let mut session = Session::new("---\ntags: []\n---\nhola\n");
    session.keys("jf]ira");
    assert_eq!(
        typed(&session),
        Some(TypedTag {
            typed: "ra".to_owned(),
            place: TagPlace::Tags,
        })
    );
    let mut session = Session::new("---\ntags: [\"\"]\n---\nhola\n");
    session.keys("jf\"lira");
    assert_eq!(typed(&session), None);
}

/// Aceptar cambia lo escrito por la forma establecida sin salir de Insert:
/// un solo paso de undo, y `.` lo repite.
#[test]
fn aceptar_es_parte_de_lo_tecleado() {
    let mut session = Session::new("hola\nchao\n");
    session.keys("A #ru");
    let pane = session.pane();
    session
        .workspace
        .complete_tag(pane, "ru", "Rust", &mut session.clipboard)
        .unwrap();
    assert_eq!(session.text(), "hola #Rust\nchao\n");
    assert_eq!(session.mode(), Mode::Insert);
    session.keys(" y⎋j.");
    assert_eq!(session.text(), "hola #Rust y\nchao #Rust y\n");
    session.keys("uu");
    assert_eq!(session.text(), "hola\nchao\n");
}
