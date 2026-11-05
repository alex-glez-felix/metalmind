//! La pregunta de borrado: sus botones enseñan la tecla del keymap y cada
//! respuesta, con ratón o teclado, hace lo que dice.

use gpui_kit::{Entity, Modifiers, TestAppContext, VisualTestContext, Window, px};

use crate::sidebar::Sidebar;
use crate::sidebar::test_support::sidebar;
use crate::theme::Theme;

fn asked(sidebar: &Entity<Sidebar>, cx: &mut VisualTestContext) -> bool {
    sidebar.read_with(cx, |sidebar, _| sidebar.panel.prompt().is_some())
}

/// El panel con la pregunta abierta sobre `a.md`, la única nota.
fn asking(cx: &mut TestAppContext) -> (tempfile::TempDir, Entity<Sidebar>, &mut VisualTestContext) {
    let (dir, sidebar, cx) = sidebar(cx);
    cx.simulate_keystrokes("j d");
    cx.run_until_parked();
    assert!(asked(&sidebar, cx));
    (dir, sidebar, cx)
}

#[test]
fn cada_boton_muestra_su_tecla_dentro() {
    let mut cx = TestAppContext::single();
    let (_dir, _sidebar, cx) = asking(&mut cx);
    for (button, key) in [("confirm-delete", "key-y"), ("cancel-delete", "key-n")] {
        let button = cx.debug_bounds(button).unwrap();
        let key = cx.debug_bounds(key).unwrap();
        assert!(button.contains(&key.center()));
    }
}

/// En el mínimo de `panel.width`, los botones no caben lado a lado.
#[test]
fn en_el_panel_mas_angosto_los_botones_se_parten_en_filas() {
    let mut cx = TestAppContext::single();
    let (_dir, _sidebar, cx) = asking(&mut cx);
    let width = px(120.);
    cx.update(|window, cx| {
        cx.global_mut::<Theme>().panel_width = width;
        window.refresh();
    });
    cx.run_until_parked();
    let cancel = cx.debug_bounds("cancel-delete").unwrap();
    assert!(cancel.right() <= width);
    assert!(cancel.top() >= cx.debug_bounds("confirm-delete").unwrap().bottom());
}

#[test]
fn las_teclas_responden_la_pregunta() {
    for (key, deletes) in [
        ("y", true),
        ("enter", true),
        ("n", false),
        ("escape", false),
    ] {
        let mut cx = TestAppContext::single();
        let (dir, sidebar, cx) = asking(&mut cx);
        cx.simulate_keystrokes(key);
        cx.run_until_parked();
        assert!(!asked(&sidebar, cx), "{key}");
        assert_eq!(!dir.path().join("a.md").exists(), deletes, "{key}");
    }
}

/// Con el foco afuera también: apretar sobre el panel se lo devuelve, y
/// sin él la acción no llegaría a la pregunta.
#[test]
fn los_botones_responden_la_pregunta() {
    for (button, deletes) in [("confirm-delete", true), ("cancel-delete", false)] {
        let mut cx = TestAppContext::single();
        let (dir, sidebar, cx) = asking(&mut cx);
        cx.update(Window::blur);
        let center = cx.debug_bounds(button).unwrap().center();
        cx.simulate_click(center, Modifiers::none());
        cx.run_until_parked();
        assert!(!asked(&sidebar, cx), "{button}");
        assert_eq!(!dir.path().join("a.md").exists(), deletes, "{button}");
    }
}
