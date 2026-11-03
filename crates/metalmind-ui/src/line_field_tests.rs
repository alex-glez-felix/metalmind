//! El campo en un panel angosto y lo que le manda la plataforma.

use gpui_kit::{AppContext as _, Bounds, Render, TestAppContext, VisualTestContext, px};
use metalmind_config::Config;
use pretty_assertions::assert_eq;

use super::*;

/// Un host mínimo: solo tiene la línea.
struct Probe {
    line: Option<InputLine>,
    field: Option<Entity<LineField<Self>>>,
    look: FieldLook,
}

impl LineHost for Probe {
    fn field_line(&self) -> Option<&InputLine> {
        self.line.as_ref()
    }

    fn edit_field(&mut self, edit: LineEdit<'_>, cx: &mut Context<Self>) {
        if let Some(line) = &mut self.line {
            line.apply(edit);
        }
        cx.notify();
    }
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::with_font(&Config::defaults().unwrap(), "Mono".into());
        let root = div().w(px(240.)).flex();
        match (&self.line, &self.field) {
            (Some(line), Some(field)) => root.child(LineField::element(
                field,
                line,
                self.look.clone(),
                &theme,
                cx,
            )),
            _ => root,
        }
    }
}

fn probe<'a>(
    cx: &'a mut TestAppContext,
    line: InputLine,
    look: FieldLook,
) -> (
    Entity<Probe>,
    Entity<LineField<Probe>>,
    &'a mut VisualTestContext,
) {
    let (host, cx) = cx.add_window_view(|_, cx| {
        let (weak, focus) = (cx.weak_entity(), cx.focus_handle());
        let field = cx.new(|_| LineField::new(weak, focus));
        Probe {
            line: Some(line),
            field: Some(field),
            look,
        }
    });
    cx.run_until_parked();
    let field = host.read_with(cx, |host, _| host.field.clone().unwrap());
    let focus = field.read_with(cx, |field, _| field.focus.clone());
    cx.update(|window, cx| window.focus(&focus, cx));
    cx.run_until_parked();
    (host, field, cx)
}

/// Las cajas del campo y de su cursor en el primer frame: la pregunta se
/// abre con el valor que ya tiene y no hay otro frame que lo corrija.
fn laid_out(input: InputLine) -> (Bounds<Pixels>, Bounds<Pixels>) {
    let mut cx = TestAppContext::single();
    let (_, _, cx) = probe(&mut cx, input, FieldLook::default());
    let field = cx.debug_bounds("line-field").unwrap();
    let caret = cx.debug_bounds("line-field-caret").unwrap();
    assert!(
        caret.left() >= field.left() && caret.right() <= field.right(),
        "el cursor {caret:?} queda fuera de {field:?}"
    );
    (field, caret)
}

/// Una lista de alias más larga que el panel: el cursor se ve al final, al
/// principio y en medio.
#[test]
fn el_cursor_se_ve_en_un_valor_mas_largo_que_el_campo() {
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
    let line = InputLine::with_cursor("rs, ferris", 2);
    let (_, _, cx) = probe(&mut cx, line, FieldLook::default());
    let before = cx.debug_bounds("line-field-before").unwrap();
    let caret = cx.debug_bounds("line-field-caret").unwrap();
    let after = cx.debug_bounds("line-field-after").unwrap();
    assert_eq!(caret.left(), before.right());
    assert_eq!(after.left(), caret.right());
    assert!(after.size.width > before.size.width, "{before:?} {after:?}");
}

/// Un preedit más largo que el campo tampoco saca al cursor.
#[test]
fn un_preedit_largo_no_saca_al_cursor() {
    let mut cx = TestAppContext::single();
    let (_, field, cx) = probe(&mut cx, InputLine::new("nota"), FieldLook::default());
    let long = "にほんごをにゅうりょくしています".repeat(8);
    field.update_in(cx, |field, window, cx| {
        field.replace_and_mark_text_in_range(None, &long, None, window, cx);
    });
    cx.run_until_parked();
    let bounds = cx.debug_bounds("line-field").unwrap();
    let caret = cx.debug_bounds("line-field-caret").unwrap();
    assert!(
        caret.right() <= bounds.right(),
        "el cursor {caret:?} queda fuera de {bounds:?}"
    );
}

#[test]
fn el_commit_de_un_preedit_va_en_el_cursor_y_lo_suelta() {
    let mut cx = TestAppContext::single();
    let line = InputLine::with_cursor("cao", 2);
    let (host, field, cx) = probe(&mut cx, line, FieldLook::default());
    field.update_in(cx, |field, window, cx| {
        field.replace_and_mark_text_in_range(None, "´", None, window, cx);
        assert_eq!(field.marked(), "´");
        assert_eq!(field.marked_text_range(window, cx), Some(2..3));
        let selected = field.selected_text_range(false, window, cx).unwrap();
        assert_eq!(selected.range, 3..3);
        field.replace_text_in_range(Some(2..3), "é", window, cx);
        assert_eq!(field.marked(), "");
        assert_eq!(field.marked_text_range(window, cx), None);
    });
    let text = host.read_with(cx, |host, _| host.line.as_ref().unwrap().text().to_owned());
    assert_eq!(text, "caéo");
}

/// Sin preedit, el IME corrige lo ya escrito: el rango viene en UTF-16.
#[test]
fn un_commit_sin_preedit_reemplaza_su_rango() {
    let mut cx = TestAppContext::single();
    let (host, field, cx) = probe(&mut cx, InputLine::new("añob"), FieldLook::default());
    field.update_in(cx, |field, window, cx| {
        field.replace_text_in_range(Some(2..4), "x", window, cx);
    });
    let text = host.read_with(cx, |host, _| host.line.as_ref().unwrap().text().to_owned());
    assert_eq!(text, "añx");
}

/// Lo que no está en la línea no se escribe en ningún lado.
#[test]
fn sin_linea_no_acepta_nada() {
    let mut cx = TestAppContext::single();
    let (host, field, cx) = probe(&mut cx, InputLine::new("nota"), FieldLook::default());
    host.update(cx, |host, _| host.line = None);
    field.update_in(cx, |field, window, cx| {
        field.replace_and_mark_text_in_range(None, "´", None, window, cx);
        assert_eq!(field.marked(), "");
        field.replace_text_in_range(None, "x", window, cx);
        assert!(field.selected_text_range(false, window, cx).is_none());
        let mut adjusted = None;
        assert_eq!(field.text_for_range(0..1, &mut adjusted, window, cx), None);
    });
}

#[test]
fn el_campo_vacio_dice_su_texto_y_al_escribir_se_ve_lo_escrito() {
    let mut cx = TestAppContext::single();
    let look = FieldLook {
        placeholder: Some("filtrar por nombre".into()),
        ..FieldLook::default()
    };
    let (_, field, cx) = probe(&mut cx, InputLine::default(), look);
    field.update_in(cx, |field, window, cx| {
        field.replace_text_in_range(None, "x", window, cx);
    });
    cx.run_until_parked();
    let before = cx.debug_bounds("line-field-before").unwrap();
    assert!(before.size.width > px(0.), "{before:?}");
}

#[test]
fn los_avisos_caen_en_su_tramo_de_texto() {
    let warnings = [1..3, 6..8];
    let ranges = |text, offset| warning_ranges(text, offset, &warnings).collect::<Vec<_>>();
    assert_eq!(ranges("abcd", 0), vec![1..3]);
    assert_eq!(ranges("efgh", 4), vec![2..4]);
    assert_eq!(ranges("ij", 8), Vec::<Range<usize>>::new());
    assert_eq!(ranges("ab", 2), vec![0..1]);
}
