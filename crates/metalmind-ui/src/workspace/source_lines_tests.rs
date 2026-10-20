//! Qué líneas muestran su fuente según dónde esté el cursor.

use metalmind_syntax::Rows;
use pretty_assertions::assert_eq;

use super::PaneId;
use super::test_session::Session;

const NOTE: &str = "\
Euler: $e^{i\\pi} + 1 = 0$
texto
$$
\\sum_{n=1}^{\\infty} \\frac{1}{n^2}
$$ y $x$ tras el cierre
fin $y$";

fn session() -> (Session, PaneId) {
    let mut session = Session::new(NOTE);
    let pane = session.pane();
    session
        .workspace
        .resize(pane, 20, Rows::uniform(1))
        .unwrap();
    (session, pane)
}

/// Las líneas que muestran la fuente, de las seis de la nota.
fn sources(session: &Session, pane: PaneId) -> Vec<usize> {
    let shown = session.view(pane).source_lines();
    (0..6).filter(|&line| shown.contains(line)).collect()
}

#[test]
fn solo_la_linea_del_cursor_muestra_la_fuente() {
    let (mut session, pane) = session();
    assert_eq!(sources(&session, pane), [0]);
    session.keys("j");
    assert_eq!(sources(&session, pane), [1]);
    session.keys("G");
    assert_eq!(sources(&session, pane), [5]);
}

#[test]
fn dentro_de_un_bloque_lo_muestra_entero() {
    let (mut session, pane) = session();
    session.keys("3G");
    assert_eq!(sources(&session, pane), [2, 3, 4]);
    session.keys("j");
    assert_eq!(sources(&session, pane), [2, 3, 4]);
    session.keys("j");
    assert_eq!(sources(&session, pane), [2, 3, 4], "la línea que lo cierra");
}

#[test]
fn entrar_y_salir_del_bloque_con_j_y_k() {
    let (mut session, pane) = session();
    session.keys("j");
    assert_eq!(sources(&session, pane), [1]);
    session.keys("j");
    assert_eq!(sources(&session, pane), [2, 3, 4], "entra por arriba");
    session.keys("3j");
    assert_eq!(sources(&session, pane), [5], "sale por abajo");
    session.keys("k");
    assert_eq!(sources(&session, pane), [2, 3, 4], "vuelve a entrar");
    session.keys("3k");
    assert_eq!(sources(&session, pane), [1], "sale por arriba");
}

#[test]
fn en_visual_las_lineas_de_la_seleccion() {
    let (mut session, pane) = session();
    session.keys("vj");
    assert_eq!(sources(&session, pane), [0, 1]);
    session.keys("o");
    assert_eq!(sources(&session, pane), [0, 1]);
    session.keys("⎋Gvk");
    assert_eq!(
        sources(&session, pane),
        [2, 3, 4, 5],
        "el cursor cierra un bloque"
    );
    session.keys("⎋");
    assert_eq!(sources(&session, pane), [2, 3, 4]);
}

#[test]
fn una_seleccion_que_sale_de_un_bloque_lo_muestra_entero() {
    let (mut session, pane) = session();
    session.keys("4Gvkk");
    assert_eq!(
        sources(&session, pane),
        [1, 2, 3, 4],
        "el ancla en el bloque"
    );
    session.keys("⎋2Gvj");
    assert_eq!(
        sources(&session, pane),
        [1, 2, 3, 4],
        "el cursor en el bloque"
    );
}

#[test]
fn el_motor_no_se_entera_de_las_formulas() {
    let (mut session, pane) = session();
    session.keys("f$;");
    let closing = "Euler: $e^{i\\pi} + 1 = 0".len();
    assert_eq!(session.cursor().get(), closing, "el `$` que cierra");
    session.keys("jjjdd");
    assert_eq!(session.view(pane).buffer().line_count(), 5);
    assert_eq!(sources(&session, pane), [2, 3]);
}
