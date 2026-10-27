//! Input handler de GPUI: por aquí llegan el texto compuesto, las commits del
//! IME y el preedit. Al IME se le muestra solo la línea del cursor (o la línea
//! de comandos), con el preedit insertado (ver [`ImeLine`]).

use std::ops::Range;

use gpui_kit::{Bounds, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window, size};

use super::Editor;
use super::cursor::cursor_position;

impl EntityInputHandler for Editor {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        let (text, adjusted) = self.ime_line(cx)?.slice(range);
        *adjusted_range = Some(adjusted);
        Some(text)
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let caret = self.ime_line(cx)?.caret();
        Some(UTF16Selection {
            range: caret..caret,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked.as_ref()?;
        Some(self.ime_line(cx)?.marked_range())
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.set_preedit(None, cx);
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.commit_text(range, text, window, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let preedit = (!new_text.is_empty()).then(|| new_text.to_owned());
        self.set_preedit(preedit, cx);
    }

    /// El IME coloca su ventana de candidatos junto al cursor.
    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let view = self.view(cx)?;
        if view.command_line.is_some() {
            return None;
        }
        let layout = self.layout.as_ref()?;
        let (laid, column) = cursor_position(&view, layout)?;
        let (origin, height) = laid.place(column)?;
        Some(Bounds::new(origin, size(height, height)))
    }

    fn character_index_for_point(
        &mut self,
        position: Point<Pixels>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        let view = self.view(cx)?;
        if view.command_line.is_some() {
            return None;
        }
        let (line, index) = self.layout.as_ref()?.hit(position)?;
        (line == view.buffer().line_of(view.cursor()))
            .then(|| self.ime_line(cx).map(|ime| ime.utf16_at(index)))
            .flatten()
    }
}
