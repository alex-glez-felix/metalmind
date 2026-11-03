//! Las teclas de edición del campo del panel. Enter y Escape son acciones del
//! registro ([`metalmind_keymap::PanelAction`]) y el texto llega por el
//! [`LineField`](crate::line_field::LineField).

use gpui_kit::{Context, KeyDownEvent, Window};

use super::Sidebar;
use crate::line_field::LineHost;
use crate::picker::{InputLine, LineEdit};

impl Sidebar {
    pub(super) fn on_key_down(
        &mut self,
        event: &KeyDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.panel.input().is_none() {
            return;
        }
        let Some(edit) = LineEdit::for_key(&event.keystroke) else {
            return;
        };
        cx.stop_propagation();
        self.edit_field(edit, cx);
    }

    /// La pregunta se cerró: su preedit no pasa a la siguiente. De una pregunta
    /// a otra no se llega sin pasar por un clic, y con él la plataforma ya
    /// confirma o suelta el preedit.
    pub(super) fn drop_preedit_without_field(&mut self, cx: &mut Context<Self>) {
        if self.panel.input().is_none() {
            self.field.update(cx, |field, _| field.clear_preedit());
        }
    }
}

impl LineHost for Sidebar {
    fn field_line(&self) -> Option<&InputLine> {
        self.panel.input()
    }

    fn edit_field(&mut self, edit: LineEdit<'_>, cx: &mut Context<Self>) {
        let Some(input) = self.panel.input_mut() else {
            return;
        };
        input.apply(edit);
        self.panel.input_changed();
        self.scroll_to_selection();
        cx.notify();
    }
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
