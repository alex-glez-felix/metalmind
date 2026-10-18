//! Input handler de GPUI para el input del picker: el texto compuesto, las
//! commits del IME y el preedit, como en el editor (ver [`ImeLine`]).

use std::ops::Range;

use gpui_kit::{Bounds, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window};

use super::{PickerItem, PickerView};
use crate::ime::ImeLine;
use crate::picker::LineEdit;
use crate::utf16;

impl<T: PickerItem> PickerView<T> {
    /// La línea del input como la ve el IME.
    fn ime_line(&self) -> ImeLine {
        let line = self.state.line();
        let marked = self.marked.as_deref().unwrap_or("");
        ImeLine::new(line.text().to_owned(), line.cursor(), marked)
    }
}

impl<T: PickerItem> EntityInputHandler for PickerView<T> {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
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
        let edit = commit(self.state.line().text(), had_preedit, range, text);
        self.edit(edit, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.marked = (!new_text.is_empty()).then(|| new_text.to_owned());
        cx.notify();
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

/// La edición de un commit del IME. Con preedit, el rango es el del preedit,
/// que no está en la línea: va en el cursor. Sin él, el rango (en UTF-16)
/// es de lo ya escrito y se reemplaza, como cuando el IME corrige una
/// palabra.
fn commit<'a>(
    line: &str,
    had_preedit: bool,
    range: Option<Range<usize>>,
    text: &'a str,
) -> LineEdit<'a> {
    match range {
        Some(range) if !had_preedit => LineEdit::Replace {
            start: utf16::to_byte(line, range.start),
            end: utf16::to_byte(line, range.end),
            text,
        },
        _ => LineEdit::Insert(text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_commit_reemplaza_lo_escrito_salvo_si_venia_de_un_preedit() {
        assert_eq!(
            commit("añob", false, Some(2..4), "x"),
            LineEdit::Replace {
                start: 3,
                end: 5,
                text: "x"
            }
        );
        assert_eq!(commit("añob", true, Some(2..3), "é"), LineEdit::Insert("é"));
        assert_eq!(commit("añob", false, None, "x"), LineEdit::Insert("x"));
    }
}
