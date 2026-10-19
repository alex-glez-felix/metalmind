//! La ventana con headings más altos que el texto, como la mide la vista:
//! en píxeles, con una línea de texto de 20 y un `#` de 48.

use metalmind_vim::{LineHeights, Rows};
use pretty_assertions::assert_eq;
use proptest::prelude::*;

use super::super::PaneId;
use super::super::test_session::Session;

const ROWS: Rows = Rows {
    text: 20,
    headings: [48, 36, 28, 24, 20, 20],
    formulas: None,
};

/// Diez líneas de texto.
const HEIGHT: usize = 200;

/// 120 líneas: un `#` cada ocho y un `##` cuatro más abajo.
fn note() -> String {
    (1..=120)
        .map(|i| match i % 8 {
            1 => format!("# Sección {i}"),
            5 => format!("## Parte {i}"),
            _ => format!("línea {i}"),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn session() -> (Session, PaneId) {
    let mut session = Session::new(&note());
    let pane = session.pane();
    session.workspace.resize(pane, HEIGHT, ROWS).unwrap();
    (session, pane)
}

/// Las líneas que se ven enteras desde arriba y la del cursor, desde 0.
fn window(session: &Session, pane: PaneId) -> (usize, usize, usize) {
    let view = session.view(pane);
    let layout = view.document.layout().unwrap();
    let top = view.viewport.top();
    let lines = view.buffer().line_count();
    let mut used = 0;
    let mut bottom = top;
    while bottom < lines && used + layout.height(bottom) <= HEIGHT {
        used += layout.height(bottom);
        bottom += 1;
    }
    (top, bottom, view.buffer().line_of(view.cursor()))
}

#[test]
fn un_heading_mide_lo_que_dice_el_tema() {
    let (session, pane) = session();
    let view = session.view(pane);
    let layout = view.document.layout().unwrap();
    assert_eq!(
        (layout.height(0), layout.height(1), layout.height(4)),
        (48, 20, 36)
    );
}

#[test]
fn con_headings_caben_menos_lineas() {
    let (mut session, pane) = session();
    // 48 + 20·3 + 36 + 20·2 = 184: la siguiente, de 20, ya no cabe.
    assert_eq!(window(&session, pane), (0, 7, 0));
    session.keys("L");
    assert_eq!(window(&session, pane).2, 6);
    session.keys("M");
    assert_eq!(window(&session, pane).2, 3);
}

#[test]
fn bajar_hasta_ver_la_linea_siguiente_mueve_lo_justo() {
    let (mut session, pane) = session();
    session.keys("7j");
    // Para que entre la 7 sale por arriba el `#` de 48, y con eso alcanza:
    // el `#` de la 8 no cabe todavía.
    assert_eq!(window(&session, pane), (1, 8, 7));
}

#[test]
fn zt_zz_y_zb_con_headings() {
    let (mut session, pane) = session();
    session.keys("40Gzt");
    assert_eq!(window(&session, pane).0, 39);
    session.keys("zb");
    let (top, bottom, cursor) = window(&session, pane);
    assert_eq!((bottom, cursor), (40, 39));
    assert!(top < cursor);
    session.keys("zz");
    let (top, bottom, cursor) = window(&session, pane);
    assert!(top < cursor && cursor < bottom);
}

#[test]
fn ctrl_d_baja_media_ventana_de_filas() {
    let (mut session, pane) = session();
    session.keys("⌃d");
    // Cinco filas de texto son 100: caben el `#`, dos líneas y no la
    // tercera (48 + 20 + 20 = 88); el cursor baja lo mismo.
    assert_eq!(window(&session, pane), (3, 10, 3));
    session.keys("⌃u");
    assert_eq!(window(&session, pane), (0, 7, 0));
}

#[test]
fn escribir_un_heading_agranda_la_linea_y_el_cursor_sigue_a_la_vista() {
    let (mut session, pane) = session();
    session.keys("L");
    assert_eq!(window(&session, pane), (0, 7, 6));
    session.keys("I# ⎋");
    let (top, bottom, cursor) = window(&session, pane);
    assert_eq!(cursor, 6);
    assert!(top <= cursor && cursor < bottom, "{top}..{bottom}");
}

fn key() -> impl Strategy<Value = &'static str> {
    prop::sample::select(vec![
        "j", "k", "5j", "9k", "G", "gg", "⌃d", "⌃u", "⌃f", "⌃b", "⌃e", "⌃y", "3⌃e", "zt", "zz",
        "zb", "H", "M", "L", "3H", "2L", "I# ⎋", "I## ⎋", "0d2l", "o# x⎋", "dd", "u",
    ])
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// Haga lo que se haga, el cursor queda en una línea que se ve entera.
    #[test]
    fn el_cursor_siempre_se_ve_entero(keys in prop::collection::vec(key(), 1..25)) {
        let (mut session, pane) = session();
        for key in keys {
            session.keys(key);
            let (top, bottom, cursor) = window(&session, pane);
            prop_assert!(
                top <= cursor && cursor < bottom.max(top + 1),
                "tras {key:?}: cursor {cursor} fuera de {top}..{bottom}"
            );
        }
    }
}
