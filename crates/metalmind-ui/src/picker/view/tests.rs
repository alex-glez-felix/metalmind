//! El input del picker con un texto que no cabe (#147): el texto se corre y
//! el cursor siempre queda dentro de la caja; un focus largo se recorta. La
//! vista previa (#77) va a la derecha si cabe, con su resultado centrado.

use gpui_kit::{Bounds, Entity, Pixels, TestAppContext, VisualTestContext};
use metalmind_config::Config;
use metalmind_keymap::Keymap;

use super::*;
use crate::actions::bind_keys;
use crate::picker::Preview;
use crate::theme::Theme;

/// Un picker sin filas: solo importa lo escrito.
#[derive(Default)]
struct Typing {
    line: InputLine,
}

impl PickerDelegate for Typing {
    fn line(&self) -> &InputLine {
        &self.line
    }

    fn len(&self) -> usize {
        0
    }

    fn selected(&self) -> usize {
        0
    }

    fn select(&mut self, _: usize) {}

    fn select_next(&mut self) {}

    fn select_previous(&mut self) {}

    fn edit(&mut self, edit: LineEdit<'_>, _: &mut Context<PickerView<Self>>) {
        self.line.apply(edit);
    }

    fn choose(&mut self, _: Open, _: &mut Context<PickerView<Self>>) -> Option<Chosen> {
        None
    }

    fn row(&mut self, _: usize) -> PickerRow {
        PickerRow::default()
    }

    fn empty(&self) -> SharedString {
        "Nada".into()
    }
}

fn picker<'a>(
    cx: &'a mut TestAppContext,
    focus: Option<&str>,
) -> (Entity<PickerView<Typing>>, &'a mut VisualTestContext) {
    with_delegate(cx, focus, Typing::default())
}

fn with_delegate<'a, D: PickerDelegate>(
    cx: &'a mut TestAppContext,
    focus: Option<&str>,
    delegate: D,
) -> (Entity<PickerView<D>>, &'a mut VisualTestContext) {
    cx.update(|cx| {
        bind_keys(Keymap::default(), cx);
        cx.set_global(Theme::with_font(
            &Config::defaults().unwrap(),
            "Mono".into(),
        ));
    });
    let options = PickerOptions {
        placeholder: "Escribe…".into(),
        splits: false,
        focus: focus.map(|focus| SharedString::from(focus.to_owned())),
    };
    cx.add_window_view(|_, cx| PickerView::new(delegate, options, cx))
}

fn edit(view: &Entity<PickerView<Typing>>, cx: &mut VisualTestContext, edit: LineEdit<'_>) {
    view.update(cx, |view, cx| {
        view.update_delegate(cx, |typing, _| typing.line.apply(edit));
    });
    cx.run_until_parked();
}

/// El cursor y la caja del texto, del último frame.
fn caret_in_box(cx: &mut VisualTestContext) -> (Bounds<Pixels>, Bounds<Pixels>) {
    let caret = cx.debug_bounds("picker-caret").unwrap();
    let text = cx.debug_bounds("picker-input-text").unwrap();
    (caret, text)
}

fn assert_visible(cx: &mut VisualTestContext) {
    let (caret, text) = caret_in_box(cx);
    assert!(
        caret.left() >= text.left() && caret.right() <= text.right(),
        "el cursor {caret:?} queda fuera de {text:?}"
    );
}

#[test]
fn el_cursor_se_ve_al_final_de_un_texto_largo_y_al_volver_al_principio() {
    let mut cx = TestAppContext::single();
    let (view, cx) = picker(&mut cx, None);
    cx.run_until_parked();
    let long = "entrevistas/2026/octubre/una entrevista con un nombre larguísimo ".repeat(6);
    edit(&view, cx, LineEdit::Insert(&long));
    assert_visible(cx);
    let (end, text) = caret_in_box(cx);
    assert!(
        end.left() > text.center().x,
        "al final, el cursor va a la derecha: {end:?} en {text:?}"
    );
    edit(&view, cx, LineEdit::Home);
    assert_visible(cx);
    let (start, text) = caret_in_box(cx);
    assert!(
        start.left() - text.left() < text.size.width / 10.,
        "al principio, el cursor vuelve a la izquierda: {start:?} en {text:?}"
    );
    edit(&view, cx, LineEdit::End);
    edit(&view, cx, LineEdit::DeleteToStart);
    assert_visible(cx);
}

/// Un focus más ancho que el picker se queda con su parte de la fila y lo
/// escrito, tras él, sigue a la vista.
#[test]
fn un_focus_largo_no_tapa_lo_escrito() {
    let mut cx = TestAppContext::single();
    let long = format!(
        "focus: Ideas/ #rust{}",
        " -#un-tag-con-nombre-largo".repeat(12)
    );
    let (view, cx) = picker(&mut cx, Some(&long));
    edit(&view, cx, LineEdit::Insert("zeta"));
    let theme = cx.update(|_, cx| Theme::get(cx).clone());
    let focus = cx.debug_bounds("picker-focus").unwrap();
    let input = cx.debug_bounds("picker-input").unwrap();
    assert!(
        focus.size.width <= input.size.width * theme.layout.picker_detail_share,
        "{focus:?} en {input:?}"
    );
    assert_visible(cx);
}

/// Un picker con una fila y la vista previa de 61 líneas con el resultado
/// en la 31.
struct Previewing {
    line: InputLine,
    preview: Preview,
}

impl Default for Previewing {
    fn default() -> Self {
        let lines = (1..=61)
            .map(|number| crate::picker::PreviewLine {
                number,
                text: format!("línea {number}"),
                highlights: Vec::new(),
            })
            .collect();
        Self {
            line: InputLine::default(),
            preview: Preview::Lines {
                lines,
                focus: Some(30),
            },
        }
    }
}

impl PickerDelegate for Previewing {
    fn line(&self) -> &InputLine {
        &self.line
    }

    fn len(&self) -> usize {
        1
    }

    fn selected(&self) -> usize {
        0
    }

    fn select(&mut self, _: usize) {}

    fn select_next(&mut self) {}

    fn select_previous(&mut self) {}

    fn edit(&mut self, edit: LineEdit<'_>, _: &mut Context<PickerView<Self>>) {
        self.line.apply(edit);
    }

    fn choose(&mut self, _: Open, _: &mut Context<PickerView<Self>>) -> Option<Chosen> {
        None
    }

    fn row(&mut self, _: usize) -> PickerRow {
        PickerRow {
            text: "una fila".to_owned(),
            ..PickerRow::default()
        }
    }

    fn empty(&self) -> SharedString {
        "Nada".into()
    }

    fn has_preview(&self) -> bool {
        true
    }

    fn preview(&self) -> Option<&Preview> {
        Some(&self.preview)
    }
}

fn previewing(cx: &mut TestAppContext, width: f32) -> &mut VisualTestContext {
    let (_, cx) = with_delegate(cx, None, Previewing::default());
    cx.simulate_resize(gpui_kit::size(gpui_kit::px(width), gpui_kit::px(1080.)));
    cx.run_until_parked();
    cx
}

/// Con lugar, la vista previa va a la derecha de la lista con su ancho del
/// tema, la lista no se encoge con una fila y el resultado queda centrado.
#[test]
fn la_vista_previa_va_a_la_derecha_con_el_resultado_al_centro() {
    let mut cx = TestAppContext::single();
    let cx = previewing(&mut cx, 1920.);
    let theme = cx.update(|_, cx| Theme::get(cx).clone());
    let preview = cx.debug_bounds("picker-preview").unwrap();
    let input = cx.debug_bounds("picker-input").unwrap();
    assert_eq!(preview.size.width, theme.layout.picker_preview_width);
    assert!(preview.left() >= input.left() + theme.layout.picker_width - gpui_kit::px(2.));
    let rows = theme.picker_row_height() * f32::from(theme.layout.picker_rows);
    assert!(preview.size.height >= rows, "{preview:?}");
    let focus = cx.debug_bounds("picker-preview-focus").unwrap();
    let off_center = (focus.center().y - preview.center().y).abs();
    assert!(off_center <= theme.ui_line(), "{focus:?} en {preview:?}");
}

/// En una ventana donde no cabe junto a la lista, no se muestra.
#[test]
fn en_una_ventana_angosta_no_hay_vista_previa() {
    let mut cx = TestAppContext::single();
    let cx = previewing(&mut cx, 1000.);
    assert!(cx.debug_bounds("picker-preview").is_none());
    assert!(cx.debug_bounds("picker-input").is_some());
}
