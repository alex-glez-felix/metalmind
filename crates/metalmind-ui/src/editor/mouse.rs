//! Ratón, rueda y portapapeles. En Linux, seleccionar con el ratón copia a
//! la selección primaria y el clic medio la pega. Un clic en un panel le da
//! el foco.

use gpui_kit::{
    App, ClipboardItem, Context, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point,
    ScrollWheelEvent, Window,
};
use metalmind_buffer::ByteOffset;

use super::Editor;
use super::formula_tooltip::hover_at;
use super::lines;
use super::rows::scroll_by_pixels;
use crate::clipboard::SystemClipboard;

impl Editor {
    fn offset_at(&self, position: Point<Pixels>, cx: &App) -> Option<ByteOffset> {
        let (line, index) = self.layout.as_ref()?.hit(position)?;
        Some(self.view(cx)?.buffer().line_start(line) + index)
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
        let Some(view) = self.view(cx) else {
            return;
        };
        let delta = event.delta.pixel_delta(line_height).y;
        self.scroll = scroll_by_pixels(
            self.scroll,
            -f64::from(delta),
            view.buffer().last_line(),
            |line| f64::from(lines::line_height(&view, line_height, line)),
        );
        let top = self.scroll.first_full_line();
        if self.synced_top != Some(top) {
            self.synced_top = Some(top);
            let scrolled =
                self.update_workspace(cx, |workspace, pane, _| workspace.scroll_to(pane, top));
            if let Err(error) = scrolled {
                self.report(&error, cx);
            }
        }
        cx.notify();
    }

    pub(super) fn on_left_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.take_focus(window, cx);
        self.completion.close();
        let Some(offset) = self.offset_at(event.position, cx) else {
            return;
        };
        self.drag_anchor = Some(offset);
        let pressed = self.update_workspace(cx, |workspace, pane, _| workspace.press(pane, offset));
        match pressed {
            Ok(on_checkbox) => self.checkbox_pressed = on_checkbox,
            Err(error) => self.report(&error, cx),
        }
    }

    pub(super) fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(anchor) = self.drag_anchor.filter(|_| event.dragging()) else {
            self.hover_formula(event.position, cx);
            return;
        };
        if self.pointer.take().is_some() {
            cx.notify();
        }
        let Some(head) = self.offset_at(event.position, cx) else {
            return;
        };
        if head != anchor {
            self.checkbox_pressed = false;
        }
        self.select(anchor, head, cx);
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
        if std::mem::take(&mut self.checkbox_pressed) {
            let toggled = self.update_workspace(cx, |workspace, pane, cx| {
                workspace.toggle_checkbox(pane, &mut SystemClipboard(cx))
            });
            if let Err(error) = toggled {
                self.report(&error, cx);
            }
            return;
        }
        let selected = self.workspace.read(cx).selected_text(self.pane);
        match selected {
            Ok(Some(text)) => cx.write_to_primary(ClipboardItem::new_string(text)),
            Ok(None) => {}
            Err(error) => self.report(&error, cx),
        }
    }

    pub(super) fn on_middle_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.take_focus(window, cx);
        if let Some(offset) = self.offset_at(event.position, cx) {
            self.select(offset, offset, cx);
        }
        let text = cx.read_from_primary().and_then(|item| item.text());
        self.paste_text(text, cx);
    }

    pub(super) fn copy(&mut self, cx: &mut Context<Self>) {
        let copied = self.update_workspace(cx, |workspace, pane, cx| {
            workspace.copy(pane, &mut SystemClipboard(cx))
        });
        if let Err(error) = copied {
            self.report(&error, cx);
        }
    }

    pub(super) fn paste(&mut self, cx: &mut Context<Self>) {
        let pasted = self.update_workspace(cx, |workspace, pane, cx| {
            workspace.paste_clipboard(pane, &mut SystemClipboard(cx))
        });
        if let Err(error) = pasted {
            self.report(&error, cx);
        }
    }

    /// El ratón se movió a `position`: si entra o sale de una fórmula que no
    /// compone, se repinta para mostrar o quitar su mensaje.
    fn hover_formula(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let layout = self.layout.as_ref();
        let changed = hover_at(layout, self.pointer) != hover_at(layout, Some(position));
        self.pointer = Some(position);
        if changed {
            cx.notify();
        }
    }

    pub(super) fn take_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus_handle, cx);
        let focused = self.update_workspace(cx, |workspace, pane, _| workspace.focus(pane));
        if let Err(error) = focused {
            self.report(&error, cx);
        }
    }

    fn select(&mut self, anchor: ByteOffset, head: ByteOffset, cx: &mut Context<Self>) {
        let selected = self.update_workspace(cx, |workspace, pane, _| {
            workspace.select(pane, anchor, head)
        });
        if let Err(error) = selected {
            self.report(&error, cx);
        }
    }

    fn paste_text(&mut self, text: Option<String>, cx: &mut Context<Self>) {
        let Some(text) = text else { return };
        let pasted = self.update_workspace(cx, |workspace, pane, _| workspace.paste(pane, &text));
        if let Err(error) = pasted {
            self.report(&error, cx);
        }
    }
}
