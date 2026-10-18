use nota_vim::Mode;
use pretty_assertions::assert_eq;

use super::super::test_session::Session;

/// `n` líneas numeradas desde 1.
fn numbered(n: usize) -> String {
    (1..=n).map(|i| i.to_string() + "\n").collect()
}

#[test]
fn ctrl_d_mueve_la_ventana_del_panel() {
    let mut session = Session::new(&numbered(100));
    let pane = session.pane();
    session.workspace.resize(pane, 10).unwrap();
    session.keys("⌃d");
    assert_eq!((session.line(pane), session.top(pane)), (6, 6));
}

#[test]
fn cada_panel_tiene_su_ventana() {
    let mut session = Session::new(&numbered(100));
    let left = session.pane();
    session.workspace.resize(left, 10).unwrap();
    session.keys("⌃wv");
    let right = session.pane();
    session.workspace.resize(right, 20).unwrap();
    session.keys("⌃d");
    assert_eq!((session.line(right), session.top(right)), (11, 11));
    assert_eq!((session.line(left), session.top(left)), (1, 1));
    session.keys("⌃wh⌃d");
    assert_eq!((session.line(left), session.top(left)), (6, 6));
    assert_eq!(session.top(right), 11);
}

#[test]
fn el_count_de_ctrl_d_es_de_cada_panel() {
    let mut session = Session::new(&numbered(100));
    let left = session.pane();
    session.workspace.resize(left, 10).unwrap();
    session.keys("⌃wv");
    let right = session.pane();
    session.workspace.resize(right, 10).unwrap();
    session.keys("2⌃d⌃wh⌃d");
    assert_eq!(session.top(right), 3);
    assert_eq!(session.top(left), 6);
}

#[test]
fn un_split_nuevo_muestra_su_cursor() {
    let mut session = Session::new(&numbered(100));
    let top = session.pane();
    session.workspace.resize(top, 40).unwrap();
    session.keys("35G⌃ws");
    let bottom = session.pane();
    session.workspace.resize(bottom, 10).unwrap();
    assert_eq!((session.line(bottom), session.top(bottom)), (35, 31));
    assert_eq!(session.top(top), 1);
}

#[test]
fn respeta_el_scrolloff_de_la_config() {
    let mut session = Session::with_scrolloff(&numbered(100), 3);
    let pane = session.pane();
    session.workspace.resize(pane, 10).unwrap();
    session.keys("6j");
    assert_eq!((session.line(pane), session.top(pane)), (7, 1));
    session.keys("j");
    assert_eq!((session.line(pane), session.top(pane)), (8, 2));
}

#[test]
fn la_rueda_lleva_el_cursor_consigo() {
    let mut session = Session::with_scrolloff(&numbered(100), 2);
    let pane = session.pane();
    session.workspace.resize(pane, 10).unwrap();
    session.workspace.scroll_to(pane, 30).unwrap();
    assert_eq!((session.line(pane), session.top(pane)), (33, 31));
    session.workspace.scroll_to(pane, 0).unwrap();
    assert_eq!((session.line(pane), session.top(pane)), (8, 1));
}

#[test]
fn la_rueda_en_visual_extiende_la_seleccion() {
    let mut session = Session::new(&numbered(100));
    let pane = session.pane();
    session.workspace.resize(pane, 10).unwrap();
    session.keys("v");
    session.workspace.scroll_to(pane, 20).unwrap();
    assert_eq!(session.mode(), Mode::Visual(nota_vim::VisualKind::Char));
    assert_eq!(session.line(pane), 21);
    session.keys("d");
    assert!(session.text().starts_with("1\n22\n"));
}

#[test]
fn la_rueda_en_insert_corta_la_repeticion() {
    let mut session = Session::new(&numbered(100));
    let pane = session.pane();
    session.workspace.resize(pane, 10).unwrap();
    session.keys("3ix");
    session.workspace.scroll_to(pane, 20).unwrap();
    session.keys("⎋");
    assert_eq!(session.text().lines().next(), Some("x1"));
}

#[test]
fn la_rueda_en_un_panel_sin_foco_no_le_da_el_foco() {
    let mut session = Session::new(&numbered(100));
    let left = session.pane();
    session.workspace.resize(left, 10).unwrap();
    session.keys("⌃wv");
    let right = session.pane();
    session.keys("i");
    session.workspace.scroll_to(left, 50).unwrap();
    assert_eq!((session.line(left), session.top(left)), (51, 51));
    assert_eq!(session.workspace.focused(), right);
    assert_eq!(session.mode(), Mode::Insert);
}

/// Lejos de la ventana, el cursor queda en medio, como en vim.
#[test]
fn una_edicion_en_otro_panel_muestra_el_cursor_que_movio() {
    let mut session = Session::new(&numbered(100));
    let left = session.pane();
    session.workspace.resize(left, 10).unwrap();
    session.keys("8G⌃wv");
    session.keys("gg20O.⎋");
    assert_eq!((session.line(left), session.top(left)), (28, 24));
}

#[test]
fn si_otro_panel_acorta_el_texto_la_ventana_no_queda_pasada_del_final() {
    let mut session = Session::new(&numbered(100));
    let left = session.pane();
    session.workspace.resize(left, 10).unwrap();
    session.keys("G⌃wv");
    session.keys("gg50dd");
    assert!(session.top(left) <= 51);
    assert!((session.top(left)..session.top(left) + 10).contains(&session.line(left)));
}

#[test]
fn cambiar_la_altura_vuelve_a_mostrar_el_cursor() {
    let mut session = Session::new(&numbered(100));
    let pane = session.pane();
    session.workspace.resize(pane, 40).unwrap();
    session.keys("30G");
    assert_eq!(session.top(pane), 1);
    session.workspace.resize(pane, 10).unwrap();
    assert_eq!(session.top(pane), 26);
}

#[test]
fn la_rueda_cancela_un_operador_pendiente() {
    let mut session = Session::new(&numbered(100));
    let pane = session.pane();
    session.workspace.resize(pane, 10).unwrap();
    session.keys("2d");
    session.workspace.scroll_to(pane, 20).unwrap();
    session.keys("j");
    assert_eq!(session.text(), numbered(100));
    assert_eq!((session.line(pane), session.mode()), (22, Mode::Normal));
}

#[test]
fn arrastrar_hasta_el_margen_no_mueve_la_ventana() {
    let mut session = Session::with_scrolloff(&numbered(100), 3);
    let pane = session.pane();
    session.workspace.resize(pane, 10).unwrap();
    let start = session.view(pane).buffer().line_start(0);
    for line in [7, 8, 9] {
        let head = session.view(pane).buffer().line_start(line);
        session.select(start, head);
        assert_eq!(
            session.top(pane),
            1,
            "arrastrando hasta la línea {}",
            line + 1
        );
    }
}
