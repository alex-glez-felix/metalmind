//! Las preguntas del panel reciben el texto por el IME, como el picker y la
//! lista, y editan con las mismas teclas.

use gpui_kit::{
    Entity, EntityInputHandler as _, Keystroke, Modifiers, TestAppContext, VisualTestContext,
};

use super::*;
use crate::panel::PanelView;
use crate::sidebar::test_support::sidebar;

/// El árbol con la pregunta de una nota nueva abierta con su tecla.
fn asking(cx: &mut TestAppContext) -> (tempfile::TempDir, Entity<Sidebar>, &mut VisualTestContext) {
    let (dir, sidebar, cx) = sidebar(cx);
    cx.simulate_keystrokes("a");
    cx.run_until_parked();
    (dir, sidebar, cx)
}

fn typed(sidebar: &Entity<Sidebar>, cx: &mut VisualTestContext) -> String {
    sidebar.read_with(cx, |sidebar, _| {
        sidebar.panel.input().unwrap().text().to_owned()
    })
}

/// `alt` hace que la tecla no sea una edición del campo: lo único que le
/// queda es el camino del input handler, el mismo por el que llega un IME.
/// Antes de #236 el panel no registraba ninguno y este texto se perdía.
#[test]
fn el_texto_de_la_plataforma_llega_al_campo() {
    let mut cx = TestAppContext::single();
    let (_dir, sidebar, cx) = asking(&mut cx);
    let keystroke = Keystroke {
        modifiers: Modifiers::alt(),
        key: "q".to_owned(),
        key_char: Some("@".to_owned()),
    };
    let handled = cx.update(|window, cx| window.dispatch_keystroke(keystroke, cx));
    assert!(handled);
    assert_eq!(typed(&sidebar, cx), "@");
}

/// Una tecla muerta: el preedit se ve subrayado y su commit queda escrito.
#[test]
fn crear_una_nota_escribe_con_el_ime() {
    let mut cx = TestAppContext::single();
    let (_dir, sidebar, cx) = asking(&mut cx);
    cx.simulate_input("caf");
    let field = sidebar.read_with(cx, |sidebar, _| sidebar.field.clone());
    field.update_in(cx, |field, window, cx| {
        field.replace_and_mark_text_in_range(None, "´", None, window, cx);
        assert_eq!(field.marked_text_range(window, cx), Some(3..4));
    });
    assert_eq!(typed(&sidebar, cx), "caf", "el preedit no está escrito");
    field.update_in(cx, |field, window, cx| {
        field.replace_text_in_range(None, "é", window, cx);
    });
    assert_eq!(typed(&sidebar, cx), "café");
}

#[test]
fn escribe_borra_y_se_mueve() {
    let mut cx = TestAppContext::single();
    let (_dir, sidebar, cx) = asking(&mut cx);
    cx.simulate_input("nota");
    cx.simulate_keystrokes("left backspace");
    assert_eq!(typed(&sidebar, cx), "noa");
    cx.simulate_keystrokes("ctrl-u");
    assert_eq!(typed(&sidebar, cx), "a");
}

/// Las preguntas del panel editan como el input del picker: por grafema y
/// con `ctrl-w`.
#[test]
fn borra_por_grafema_y_por_palabra() {
    let mut cx = TestAppContext::single();
    let (_dir, sidebar, cx) = asking(&mut cx);
    let field = sidebar.read_with(cx, |sidebar, _| sidebar.field.clone());
    field.update_in(cx, |field, window, cx| {
        field.replace_text_in_range(None, "cafe\u{301}", window, cx);
    });
    cx.simulate_keystrokes("backspace");
    assert_eq!(
        typed(&sidebar, cx),
        "caf",
        "el acento combinante se va con su e"
    );
    cx.simulate_keystrokes("ctrl-u");
    cx.simulate_input("diario");
    cx.simulate_keystrokes("ctrl-w");
    assert_eq!(typed(&sidebar, cx), "");
}

/// Escribir en el filtro filtra el árbol, venga de donde venga el texto.
#[test]
fn el_texto_del_ime_filtra_el_arbol() {
    let mut cx = TestAppContext::single();
    let (_dir, sidebar, cx) = sidebar(&mut cx);
    sidebar.update(cx, |sidebar, cx| {
        sidebar.panel.start_filter();
        cx.notify();
    });
    cx.run_until_parked();
    let field = sidebar.read_with(cx, |sidebar, _| sidebar.field.clone());
    field.update_in(cx, |field, window, cx| {
        field.replace_text_in_range(None, "zzz", window, cx);
    });
    let filter = sidebar.read_with(cx, |sidebar, _| sidebar.panel.tree.filter().to_owned());
    assert_eq!(filter, "zzz");
}

#[test]
fn cerrar_la_pregunta_suelta_el_preedit() {
    let mut cx = TestAppContext::single();
    let (_dir, sidebar, cx) = asking(&mut cx);
    let field = sidebar.read_with(cx, |sidebar, _| sidebar.field.clone());
    field.update_in(cx, |field, window, cx| {
        field.replace_and_mark_text_in_range(None, "´", None, window, cx);
    });
    cx.simulate_keystrokes("escape");
    assert_eq!(
        field.read_with(cx, |field, _| field.marked().to_owned()),
        ""
    );
}

/// Cambiar de vista desde fuera del panel también cierra la pregunta.
#[test]
fn cambiar_de_vista_suelta_el_preedit() {
    let mut cx = TestAppContext::single();
    let (_dir, sidebar, cx) = asking(&mut cx);
    let field = sidebar.read_with(cx, |sidebar, _| sidebar.field.clone());
    field.update_in(cx, |field, window, cx| {
        field.replace_and_mark_text_in_range(None, "´", None, window, cx);
    });
    sidebar.update_in(cx, |sidebar, window, cx| {
        sidebar.toggle(PanelView::Tags, window, cx);
        assert!(sidebar.panel.input().is_none());
    });
    assert_eq!(
        field.read_with(cx, |field, _| field.marked().to_owned()),
        ""
    );
}
