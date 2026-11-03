//! Lo que se escribe en el filtro y en los tags de la lista: las teclas de
//! edición, como en readline; el texto llega por el
//! [`LineField`](crate::line_field::LineField).

use gpui_kit::{Context, KeyDownEvent, Window};

use super::model::Prompt;
use super::view::NoteListView;
use crate::line_field::LineHost;
use crate::picker::{InputLine, LineEdit};

impl NoteListView {
    /// Las teclas de edición del campo; `enter` y `esc` son acciones del
    /// registro.
    pub(super) fn on_key_down(
        &mut self,
        event: &KeyDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.list.prompt().is_none() {
            return;
        }
        let Some(edit) = LineEdit::for_key(&event.keystroke) else {
            return;
        };
        cx.stop_propagation();
        self.edit_prompt(edit, cx);
    }
}

impl LineHost for NoteListView {
    fn field_line(&self) -> Option<&InputLine> {
        match self.list.prompt()? {
            Prompt::Filter => Some(self.list.filter()),
            Prompt::Tag => Some(self.list.tag_line()),
        }
    }

    fn edit_field(&mut self, edit: LineEdit<'_>, cx: &mut Context<Self>) {
        self.edit_prompt(edit, cx);
    }
}
