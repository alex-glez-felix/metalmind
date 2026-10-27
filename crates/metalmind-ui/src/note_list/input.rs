//! Lo que se escribe en el filtro y en los tags de la lista: las teclas de
//! edición, como en readline, y el texto, que llega por el IME (también las
//! teclas muertas: `´` + `a` = `á`), como en el input del picker.

use std::ops::Range;

use gpui_kit::{
    Bounds, Context, EntityInputHandler, KeyDownEvent, Keystroke, Pixels, Point, UTF16Selection,
    Window,
};

use super::model::Prompt;
use super::view::NoteListView;
use crate::ime::{ImeLine, commit};
use crate::picker::{InputLine, LineEdit};

impl NoteListView {
    /// Las teclas de edición del campo; `enter` y `esc` son acciones del
    /// registro y el texto llega por [`EntityInputHandler`].
    pub(super) fn on_key_down(
        &mut self,
        event: &KeyDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.list.prompt().is_none() {
            return;
        }
        let Some(edit) = line_edit(&event.keystroke) else {
            return;
        };
        cx.stop_propagation();
        self.edit_prompt(edit, cx);
    }

    fn line(&self) -> Option<&InputLine> {
        match self.list.prompt()? {
            Prompt::Filter => Some(self.list.filter()),
            Prompt::Tag => Some(self.list.tag_line()),
        }
    }

    fn ime_line(&self) -> ImeLine {
        let marked = self.marked.as_deref().unwrap_or("");
        match self.line() {
            Some(line) => ImeLine::new(line.text().to_owned(), line.cursor(), marked),
            None => ImeLine::new(String::new(), 0, ""),
        }
    }
}

/// La tecla de edición que pide `keystroke`, si es una.
fn line_edit(keystroke: &Keystroke) -> Option<LineEdit<'static>> {
    let modifiers = keystroke.modifiers;
    if modifiers.alt || modifiers.platform || modifiers.function {
        return None;
    }
    if modifiers.control {
        return match keystroke.key.as_str() {
            "u" => Some(LineEdit::DeleteToStart),
            "w" | "backspace" => Some(LineEdit::DeleteWordBackward),
            "a" => Some(LineEdit::Home),
            "e" => Some(LineEdit::End),
            _ => None,
        };
    }
    match keystroke.key.as_str() {
        "backspace" => Some(LineEdit::DeleteBackward),
        "delete" => Some(LineEdit::DeleteForward),
        "left" => Some(LineEdit::Left),
        "right" => Some(LineEdit::Right),
        "home" => Some(LineEdit::Home),
        "end" => Some(LineEdit::End),
        _ => None,
    }
}

impl EntityInputHandler for NoteListView {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        self.line()?;
        let (text, adjusted) = self.ime_line().slice(range);
        *adjusted_range = Some(adjusted);
        Some(text)
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        self.line()?;
        let caret = self.ime_line().caret();
        Some(UTF16Selection {
            range: caret..caret,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked.as_ref()?;
        Some(self.ime_line().marked_range())
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let had_preedit = self.marked.take().is_some();
        let Some(line) = self.line().map(|line| line.text().to_owned()) else {
            return;
        };
        let edit = commit(&line, had_preedit, range, text);
        self.edit_prompt(edit, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.line().is_some() {
            self.marked = (!new_text.is_empty()).then(|| new_text.to_owned());
            cx.notify();
        }
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(element_bounds)
    }

    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}
