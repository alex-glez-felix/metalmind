use gpui_kit::{Bounds, IntoElement, Pixels, Render, TestAppContext, Window, px};
use metalmind_config::Config;

use super::*;

/// El campo de una pregunta en un panel angosto.
struct FieldProbe(InputLine);

impl Render for FieldProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::with_font(&Config::defaults().unwrap(), "Mono".into());
        div().w(px(240.)).child(input_field(&self.0, &theme))
    }
}

/// Las cajas del campo y de su cursor en el primer frame: la pregunta se
/// abre con el valor que ya tiene y no hay otro frame que lo corrija.
fn laid_out(input: InputLine) -> (Bounds<Pixels>, Bounds<Pixels>) {
    let mut cx = TestAppContext::single();
    let (_, cx) = cx.add_window_view(|_, _| FieldProbe(input));
    cx.run_until_parked();
    let field = cx.debug_bounds("panel-input").unwrap();
    let caret = cx.debug_bounds("panel-caret").unwrap();
    assert!(
        caret.left() >= field.left() && caret.right() <= field.right(),
        "el cursor {caret:?} queda fuera de {field:?}"
    );
    (field, caret)
}

/// Una lista de alias más larga que el panel: el cursor se ve al final, al
/// principio y en medio.
#[test]
fn el_cursor_se_ve_en_un_valor_mas_largo_que_el_panel() {
    let long = "rs, rust-lang, rustacean, ferris, cargo, crates, oxidación, ".repeat(4);
    let (field, end) = laid_out(InputLine::new(&long));
    assert!(
        end.left() > field.center().x,
        "al final, el cursor va a la derecha: {end:?} en {field:?}"
    );
    let (field, start) = laid_out(InputLine::with_cursor(&long, 0));
    assert!(
        start.left() - field.left() < field.size.width / 10.,
        "al principio, el cursor va a la izquierda: {start:?} en {field:?}"
    );
    laid_out(InputLine::with_cursor(&long, long.len() / 2));
}

/// Lo que cabe se ve entero y el cursor queda justo tras lo escrito.
#[test]
fn un_valor_corto_no_se_recorta() {
    let mut cx = TestAppContext::single();
    let (_, cx) = cx.add_window_view(|_, _| FieldProbe(InputLine::with_cursor("rs, ferris", 2)));
    cx.run_until_parked();
    let before = cx.debug_bounds("panel-input-before").unwrap();
    let caret = cx.debug_bounds("panel-caret").unwrap();
    let after = cx.debug_bounds("panel-input-after").unwrap();
    assert_eq!(caret.left(), before.right());
    assert_eq!(after.left(), caret.right());
    assert!(after.size.width > before.size.width, "{before:?} {after:?}");
}
