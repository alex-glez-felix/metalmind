//! El input del picker con un texto que no cabe (#147): el texto se corre y
//! el cursor siempre queda dentro de la caja.

use gpui_kit::{Bounds, Entity, Pixels, TestAppContext, VisualTestContext};
use metalmind_config::Config;
use metalmind_keymap::Keymap;

use super::*;
use crate::actions::bind_keys;
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

fn picker(cx: &mut TestAppContext) -> (Entity<PickerView<Typing>>, &mut VisualTestContext) {
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
    };
    cx.add_window_view(|_, cx| PickerView::new(Typing::default(), options, cx))
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
    let (view, cx) = picker(&mut cx);
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
