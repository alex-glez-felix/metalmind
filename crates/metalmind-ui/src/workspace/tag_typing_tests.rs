//! El tag que se está escribiendo y aceptar su autocompletado (05 §3).

use std::fmt::Write as _;

use metalmind_syntax::TagPlace;
use metalmind_vim::Mode;
use pretty_assertions::assert_eq;

use super::tag_typing::TypedTag;
use crate::workspace::test_session::Session;

fn typed(session: &Session) -> Option<TypedTag> {
    session.workspace.typed_tag(session.pane()).unwrap()
}

/// Lo escrito tras `hola #`, en la primera línea.
fn inline(typed: &str) -> TypedTag {
    TypedTag {
        typed: typed.to_owned(),
        range: 6..6 + typed.len(),
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
            range: 11..13,
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
        .complete_tag(pane, "Rust", &mut session.clipboard)
        .unwrap();
    assert_eq!(session.text(), "hola #Rust\nchao\n");
    assert_eq!(session.mode(), Mode::Insert);
    session.keys(" y⎋j.");
    assert_eq!(session.text(), "hola #Rust y\nchao #Rust y\n");
    session.keys("uu");
    assert_eq!(session.text(), "hola\nchao\n");
}

/// En Insert por bloque se escribe en la primera línea y `⎋` lo copia a
/// las demás, ya completado.
#[test]
fn en_insert_por_bloque() {
    let mut session = Session::new("hola\nchao\n");
    session.keys("⌃vj$A #ru");
    assert_eq!(typed(&session), Some(inline("ru")));
    let pane = session.pane();
    session
        .workspace
        .complete_tag(pane, "Rust", &mut session.clipboard)
        .unwrap();
    session.keys("⎋");
    assert_eq!(session.text(), "hola #Rust\nchao #Rust\n");
}

/// Un tag que ya estaba antes de un Insert sobre un bloque no se completa:
/// borrarlo no se repetiría en las demás líneas.
#[test]
fn en_insert_por_bloque_no_lo_que_ya_estaba() {
    let mut session = Session::new("hola #ru\nchao\n");
    session.keys("⌃vj$A");
    assert_eq!(typed(&session), None);
}

/// Aceptar mira el tag de ahora: si ya no se escribe uno (otro panel, fuera
/// de Insert), no cambia nada.
#[test]
fn aceptar_sin_tag_a_medio_escribir_no_cambia_nada() {
    let mut session = Session::new("hola\n");
    session.keys("A #ru⎋");
    let pane = session.pane();
    session
        .workspace
        .complete_tag(pane, "Rust", &mut session.clipboard)
        .unwrap();
    assert_eq!(session.text(), "hola #ru\n");
}

/// El panel sin el foco no escribe: su tag no se completa.
#[test]
fn en_otro_panel_no() {
    let mut session = Session::new("hola\n");
    let first = session.pane();
    session.keys(":vsplit⏎A #ru");
    assert_eq!(typed(&session), Some(inline("ru")));
    let second = session.pane();
    session.workspace.focus(first).unwrap();
    assert_eq!(session.workspace.typed_tag(second).unwrap(), None);
}

/// El `---` de cierre de un frontmatter largo queda lejos del cursor, pero
/// el frontmatter se estiliza entero: el tag se completa igual.
#[test]
fn en_un_frontmatter_largo() {
    let mut text = String::from("---\ntags: []\n");
    for field in 0..2000 {
        writeln!(text, "campo{field}: {field}").unwrap();
    }
    text.push_str("---\nhola\n");
    let mut session = Session::new(&text);
    session.keys("jf]ira");
    assert_eq!(
        typed(&session),
        Some(TypedTag {
            typed: "ra".to_owned(),
            range: 11..13,
            place: TagPlace::Tags,
        })
    );
}

/// Con el principio del tag escrito antes de entrar en Insert, `.` repite
/// lo que nvim: con `ignorecase`, `ru` y `ctrl-n` hasta `Rust` (o `rust`)
/// dejan `abRust` (o `abcdst`) al repetirse tras `abcd`.
#[rstest::rstest]
#[case::cambia_lo_escrito("Rust", "#Rust\nabRust\n")]
#[case::sigue_a_lo_escrito("rust", "#rust\nabcdst\n")]
fn el_punto_repite_como_el_completado_de_nvim(#[case] text: &str, #[case] expected: &str) {
    let mut session = Session::new("#ru\nabcd\n");
    session.keys("A");
    let pane = session.pane();
    session
        .workspace
        .complete_tag(pane, text, &mut session.clipboard)
        .unwrap();
    session.keys("⎋j.");
    assert_eq!(session.text(), expected);
}
