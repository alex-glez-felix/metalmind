//! El wikilink que se está escribiendo y aceptar su autocompletado.

use metalmind_vim::Mode;
use pretty_assertions::assert_eq;

use super::link_typing::TypedLink;
use crate::workspace::test_session::Session;

fn typed(session: &Session) -> Option<TypedLink> {
    session.workspace.typed_link(session.pane()).unwrap()
}

fn open(typed: &str) -> TypedLink {
    TypedLink {
        typed: typed.to_owned(),
        closed: false,
    }
}

fn accept(session: &mut Session, target: &str) {
    let pane = session.pane();
    session
        .workspace
        .complete_link(pane, target, &mut session.clipboard)
        .unwrap();
}

/// Desde el `[[`, lo que va del destino; solo en Insert.
#[test]
fn desde_el_doble_corchete_y_solo_en_insert() {
    let mut session = Session::new("hola\n");
    session.keys("A [[");
    assert_eq!(typed(&session), Some(open("")));
    session.keys("pla");
    assert_eq!(typed(&session), Some(open("pla")));
    session.keys("⎋");
    assert_eq!(typed(&session), None, "en Normal no se completa");
}

#[rstest::rstest]
#[case::un_corchete("A [pla")]
#[case::en_la_subruta("A [[plan#Me")]
#[case::en_el_alias("A [[plan|el")]
fn lo_que_no_es_un_destino(#[case] keys: &str) {
    let mut session = Session::new("hola\n");
    session.keys(keys);
    assert_eq!(typed(&session), None);
}

/// Dentro de código inline, que ya cierra su `` ` ``, no.
#[test]
fn en_codigo_no() {
    let mut session = Session::new("`` y\n");
    session.keys("a[[pla");
    assert_eq!(session.text(), "`[[pla` y\n");
    assert_eq!(typed(&session), None);
    let mut session = Session::new("```\n\n```\n");
    session.keys("ji[[pla");
    assert_eq!(typed(&session), None);
}

/// Aceptar escribe el destino y cierra el link sin salir de Insert: un
/// solo paso de undo, y `.` lo repite.
#[test]
fn aceptar_cierra_el_link_y_es_parte_de_lo_tecleado() {
    let mut session = Session::new("hola\nchao\n");
    session.keys("A [[pla");
    accept(&mut session, "proyectos/plan");
    assert_eq!(session.text(), "hola [[proyectos/plan]]\nchao\n");
    assert_eq!(session.mode(), Mode::Insert);
    assert_eq!(typed(&session), None, "tras el `]]` ya no hay lista");
    session.keys(" y⎋j.");
    assert_eq!(
        session.text(),
        "hola [[proyectos/plan]] y\nchao [[proyectos/plan]] y\n"
    );
    session.keys("uu");
    assert_eq!(session.text(), "hola\nchao\n");
}

/// Si lo escrito empieza el destino, solo se teclea el resto.
#[test]
fn si_lo_escrito_empieza_el_destino_solo_se_teclea_el_resto() {
    let mut session = Session::new("\n");
    session.keys("i[[pl");
    accept(&mut session, "plan");
    assert_eq!(session.text(), "[[plan]]\n");
}

/// Con el `]]` ya escrito, aceptar no lo repite.
#[test]
fn con_el_link_cerrado_no_repite_el_cierre() {
    let mut session = Session::new("ver [[]] y\n");
    session.keys("f]i");
    assert_eq!(
        typed(&session),
        Some(TypedLink {
            typed: String::new(),
            closed: true,
        })
    );
    session.keys("pla");
    accept(&mut session, "plan");
    assert_eq!(session.text(), "ver [[plan]] y\n");
}

/// Un embed se completa igual.
#[test]
fn un_embed() {
    let mut session = Session::new("\n");
    session.keys("i![[fo");
    accept(&mut session, "foto.png");
    assert_eq!(session.text(), "![[foto.png]]\n");
}

#[test]
fn aceptar_sin_link_a_medio_escribir_no_cambia_nada() {
    let mut session = Session::new("hola\n");
    session.keys("A [[pla⎋");
    accept(&mut session, "plan");
    assert_eq!(session.text(), "hola [[pla\n");
}

/// En Insert sobre un bloque, lo escrito en la pasada se completa y `⎋` lo
/// repite ya completado; lo que ya estaba antes no, porque borrarlo no se
/// repetiría en las demás líneas.
#[test]
fn en_insert_por_bloque_solo_lo_escrito_en_la_pasada() {
    let mut session = Session::new("a\nb\n");
    session.keys("⌃vj$A [[pl");
    assert_eq!(typed(&session), Some(open("pl")));
    accept(&mut session, "plan");
    session.keys("⎋");
    assert_eq!(session.text(), "a [[plan]]\nb [[plan]]\n");
    let mut session = Session::new("a [[pl\nb\n");
    session.keys("⌃vj$A");
    assert_eq!(typed(&session), None);
}

/// Un clic dentro de un link que ya estaba termina la pasada por bloque
/// (como en vim, mover el cursor la corta): completarlo ya no se repite en
/// las demás líneas.
#[test]
fn un_clic_en_insert_por_bloque_corta_la_repeticion() {
    let mut session = Session::new("x [[pl y\nb\n");
    session.keys("⌃vjIz");
    let pane = session.pane();
    let after_pl = metalmind_buffer::ByteOffset::new("zx [[pl".len());
    session.workspace.select(pane, after_pl, after_pl).unwrap();
    assert_eq!(typed(&session), Some(open("pl")));
    accept(&mut session, "plan");
    session.keys("⎋");
    assert_eq!(session.text(), "zx [[plan]] y\nb\n");
}
