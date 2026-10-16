//! Vista propia: `nota-buffer` + `nota-vim` con un render básico.

mod element;
mod input;
mod layout;

use std::ops::Range;

use gpui_kit::{
    Action, ClipboardItem, Context, CursorStyle, FocusHandle, InteractiveElement as _, IntoElement,
    KeyBinding, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ParentElement as _, Pixels, Point, Render, ScrollWheelEvent, Styled as _, Window, div,
};
use nota_buffer::ByteOffset;
use nota_vim::{Key, KeyOutcome, Mode};

use self::element::EditorElement;
use self::input::ImeLine;
use self::layout::FrameLayout;
use crate::document::{Document, DocumentError};
use crate::events::EventLog;
use crate::hud::Status;
use crate::keys::{self, Route};
use crate::scroll::Scroll;
use crate::tokens::Tokens;

const CONTEXT: &str = "VimEditor";

/// Copia la selección del ratón, o la línea del cursor si no hay.
#[derive(Clone, PartialEq, Eq, Default, Debug, Action)]
#[action(namespace = editor, name = "copy")]
pub struct Copy;

#[derive(Clone, PartialEq, Eq, Default, Debug, Action)]
#[action(namespace = editor, name = "paste")]
pub struct Paste;

pub fn bind_keys(cx: &mut gpui_kit::App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-shift-c", Copy, Some(CONTEXT)),
        KeyBinding::new("ctrl-shift-v", Paste, Some(CONTEXT)),
    ]);
}

pub struct VimEditor {
    document: Document,
    focus_handle: FocusHandle,
    scroll: Scroll,
    layout: Option<FrameLayout>,
    marked: Option<String>,
    events: EventLog,
    drag_anchor: Option<ByteOffset>,
}

impl VimEditor {
    pub fn new(text: &str, cx: &mut Context<Self>) -> Self {
        Self {
            document: Document::new(text),
            focus_handle: cx.focus_handle(),
            scroll: Scroll::default(),
            layout: None,
            marked: None,
            events: EventLog::default(),
            drag_anchor: None,
        }
    }

    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus_handle
    }

    pub fn status(&self) -> Status {
        let buffer = self.document.buffer();
        let cursor = self.document.cursor();
        Status {
            mode: mode_name(self.document.mode()),
            line: buffer.line_of(cursor) + 1,
            column: buffer.column_of(cursor) + 1,
            lines: buffer.line_count(),
            preedit: self.marked.clone(),
            events: self.events.recent().map(str::to_owned).collect(),
        }
    }

    /// Para el benchmark: scroll sin mover el cursor.
    pub fn scroll_lines(&mut self, lines: f64, cx: &mut Context<Self>) {
        self.scroll
            .scroll_by(lines, self.document.buffer().last_line());
        cx.notify();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let route = keys::route(keystroke, self.document.mode());
        let raw = format!("{} {:?}", keystroke.unparse(), keystroke.key_char);
        let entry = match route {
            Route::Engine(key) => {
                cx.stop_propagation();
                self.marked = None;
                format!("{raw} → {key:?} → {}", self.feed(key))
            }
            Route::TextInput => format!("{raw} → texto, va por el input handler"),
            Route::Ignored => format!("{raw} → sin equivalente en nota-vim"),
        };
        self.events.push(entry);
        self.after_change(cx);
    }

    fn feed(&mut self, key: Key) -> String {
        describe(self.document.feed(key))
    }

    fn commit_text(&mut self, range: Option<Range<usize>>, text: &str, cx: &mut Context<Self>) {
        let marked_range = self
            .ime_line()
            .filter(|_| self.marked.is_some())
            .map(|ime| ime.marked_range());
        self.marked = None;
        if let Some(range) = range.filter(|range| Some(range) != marked_range.as_ref()) {
            self.events.push(format!(
                "IME pidió reemplazar {range:?}; el spike solo escribe en el cursor"
            ));
        }
        if !text.is_empty() {
            let outcome = self
                .document
                .type_text(text)
                .map(|last| last.unwrap_or(KeyOutcome::Unbound));
            self.events.push(format!(
                "texto {text:?} (input handler) → {}",
                describe(outcome)
            ));
        }
        self.after_change(cx);
    }

    fn set_preedit(&mut self, preedit: Option<String>, cx: &mut Context<Self>) {
        self.events.push(format!("preedit {preedit:?}"));
        self.marked = preedit;
        cx.notify();
    }

    fn ime_line(&self) -> Option<ImeLine> {
        let buffer = self.document.buffer();
        let cursor = self.document.cursor();
        let line = buffer.line_of(cursor);
        let text = self.document.line_text(line).ok()?;
        let column = cursor.get() - buffer.line_start(line).get();
        Some(ImeLine::new(
            text,
            column,
            self.marked.as_deref().unwrap_or(""),
        ))
    }

    fn after_change(&mut self, cx: &mut Context<Self>) {
        if let Some(layout) = &self.layout {
            let line = self.document.buffer().line_of(self.document.cursor());
            self.scroll.reveal(line, layout.viewport.full_rows());
        }
        cx.notify();
    }

    fn offset_at(&self, position: Point<Pixels>) -> Option<ByteOffset> {
        let (line, index) = self.layout.as_ref()?.hit(position)?;
        Some(self.document.buffer().line_start(line) + index)
    }

    fn on_scroll(&mut self, event: &ScrollWheelEvent, window: &mut Window, cx: &mut Context<Self>) {
        let line_height = window.line_height();
        let delta = event.delta.pixel_delta(line_height).y;
        self.scroll_lines(-f64::from(delta / line_height), cx);
    }

    fn on_left_down(
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

    fn on_mouse_move(
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

    /// En Linux seleccionar con el ratón copia a la selección primaria.
    fn on_left_up(&mut self, _event: &MouseUpEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if self.drag_anchor.take().is_none() {
            return;
        }
        match self.document.selected_text() {
            Ok(Some(text)) => {
                self.events.push(format!("primaria ← {} bytes", text.len()));
                cx.write_to_primary(ClipboardItem::new_string(text));
            }
            Ok(None) => {}
            Err(error) => self.report_error(&error),
        }
    }

    fn on_middle_down(
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
        self.paste_text("primaria", text, cx);
    }

    fn copy(&mut self, _: &Copy, _window: &mut Window, cx: &mut Context<Self>) {
        let text = self
            .document
            .selected_text()
            .transpose()
            .unwrap_or_else(|| self.document.current_line());
        match text {
            Ok(text) => {
                self.events
                    .push(format!("portapapeles ← {} bytes", text.len()));
                cx.write_to_clipboard(ClipboardItem::new_string(text));
            }
            Err(error) => self.report_error(&error),
        }
    }

    fn paste(&mut self, _: &Paste, _window: &mut Window, cx: &mut Context<Self>) {
        let text = cx.read_from_clipboard().and_then(|item| item.text());
        self.paste_text("portapapeles", text, cx);
    }

    fn paste_text(&mut self, source: &str, text: Option<String>, cx: &mut Context<Self>) {
        let Some(text) = text else {
            self.events.push(format!("{source} vacío"));
            return;
        };
        match self.document.paste(&text) {
            Ok(()) => self
                .events
                .push(format!("{source} → {} bytes pegados", text.len())),
            Err(error) => self.report_error(&error),
        }
        self.after_change(cx);
    }

    fn report_error(&mut self, error: &DocumentError) {
        log::error!("{error}");
        self.events.push(format!("ERROR: {error}"));
    }
}

fn describe(outcome: Result<KeyOutcome, DocumentError>) -> String {
    match outcome {
        Ok(KeyOutcome::Executed(action)) => action.name().to_owned(),
        Ok(KeyOutcome::Pending) => "pendiente".to_owned(),
        Ok(KeyOutcome::Inserted) => "insertado".to_owned(),
        Ok(KeyOutcome::Unbound) => "sin binding".to_owned(),
        Err(error) => {
            log::error!("{error}");
            format!("ERROR: {error}")
        }
    }
}

fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Normal => "NORMAL",
        Mode::Insert => "INSERT",
        Mode::OperatorPending(_) => "OPERADOR",
    }
}

impl Render for VimEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tokens = Tokens::from_app(cx);
        div()
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .size_full()
            .overflow_hidden()
            .cursor(CursorStyle::IBeam)
            .font_family(tokens.mono_font_family.clone())
            .text_size(tokens.mono_font_size)
            .text_color(tokens.foreground)
            .bg(tokens.background)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::paste))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_left_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_left_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_left_up))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_middle_down))
            .child(EditorElement::new(cx.entity(), tokens))
    }
}
