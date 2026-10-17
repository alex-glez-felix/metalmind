//! Ratón, rueda y portapapeles. En Linux, seleccionar con el ratón copia a
//! la selección primaria y el clic medio la pega.

use gpui_kit::{
    ClipboardItem, Context, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point,
    ScrollWheelEvent, Window,
};
use nota_buffer::ByteOffset;

use super::Editor;
use crate::actions::{Copy, Paste};

impl Editor {
    fn offset_at(&self, position: Point<Pixels>) -> Option<ByteOffset> {
        let (line, index) = self.layout.as_ref()?.hit(position)?;
        Some(self.document.buffer().line_start(line) + index)
    }

    pub(super) fn on_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(line_height) = self.layout.as_ref().map(|l| l.viewport.line_height) else {
            return;
        };
        let delta = event.delta.pixel_delta(line_height).y;
        let last_line = self.document.buffer().last_line();
        self.scroll
            .scroll_by(-f64::from(delta / line_height), last_line);
        cx.notify();
    }

    pub(super) fn on_left_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle, cx);
        let Some(offset) = self.offset_at(event.position) else {
            return;
        };
        self.drag_anchor = Some(offset);
        self.document.select(offset, offset);
        cx.notify();
    }

    pub(super) fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(anchor) = self.drag_anchor.filter(|_| event.dragging()) else {
            return;
        };
        let Some(head) = self.offset_at(event.position) else {
            return;
        };
        self.document.select(anchor, head);
        cx.notify();
    }

    pub(super) fn on_left_up(
        &mut self,
        _event: &MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.drag_anchor.take().is_none() {
            return;
        }
        match self.document.selected_text() {
            Ok(Some(text)) => cx.write_to_primary(ClipboardItem::new_string(text)),
            Ok(None) => {}
            Err(error) => self.report_error(&error),
        }
    }

    pub(super) fn on_middle_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle, cx);
        if let Some(offset) = self.offset_at(event.position) {
            self.document.select(offset, offset);
        }
        let text = cx.read_from_primary().and_then(|item| item.text());
        self.paste_text(text, cx);
    }

    pub(super) fn copy(&mut self, _: &Copy, _window: &mut Window, cx: &mut Context<Self>) {
        let text = self
            .document
            .selected_text()
            .transpose()
            .unwrap_or_else(|| self.document.current_line());
        match text {
            Ok(text) => cx.write_to_clipboard(ClipboardItem::new_string(text)),
            Err(error) => self.report_error(&error),
        }
    }

    pub(super) fn paste(&mut self, _: &Paste, _window: &mut Window, cx: &mut Context<Self>) {
        let text = cx.read_from_clipboard().and_then(|item| item.text());
        self.paste_text(text, cx);
    }

    fn paste_text(&mut self, text: Option<String>, cx: &mut Context<Self>) {
        let Some(text) = text else { return };
        if let Err(error) = self.document.paste(&text) {
            self.report_error(&error);
        }
        self.reveal_cursor(cx);
    }
}
