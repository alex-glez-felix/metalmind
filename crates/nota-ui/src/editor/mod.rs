//! Vista de editor: `nota-buffer` + `nota-vim` pintando solo lo visible.

mod clipboard;
mod commands;
mod element;
mod input;
mod keys;
mod layout;
mod mouse;
mod scroll;
mod status_bar;
mod utf16;

use std::ops::Range;
use std::time::Instant;

use gpui_kit::{
    Context, CursorStyle, FocusHandle, InteractiveElement as _, IntoElement, KeyDownEvent,
    MouseButton, ParentElement as _, Render, SharedString, Styled as _, Window, div, relative,
};
use nota_buffer::ByteOffset;
use nota_vim::{Action, KeyOutcome};

use self::clipboard::SystemClipboard;
use self::element::EditorElement;
use self::input::ImeLine;
use self::keys::Route;
use self::layout::FrameLayout;
use self::scroll::Scroll;
use self::status_bar::{Message, Status, status_bar};
use crate::actions::{self, EDITOR_CONTEXT};
use crate::document::{Document, DocumentError};
use crate::theme::Theme;

pub struct Editor {
    document: Document,
    focus_handle: FocusHandle,
    scroll: Scroll,
    /// Lo pintado en el último frame, para el ratón y el IME.
    layout: Option<FrameLayout>,
    /// Preedit del IME o de una tecla muerta.
    marked: Option<String>,
    drag_anchor: Option<ByteOffset>,
    message: Option<Message>,
    /// Arranque del proceso, hasta que se pinta el primer frame.
    launched: Option<Instant>,
    title: String,
}

impl Editor {
    pub fn new(document: Document, launched: Instant, cx: &mut Context<Self>) -> Self {
        let message = opened_message(&document);
        Self {
            document,
            focus_handle: cx.focus_handle(),
            scroll: Scroll::default(),
            layout: None,
            marked: None,
            drag_anchor: None,
            message: Some(message),
            launched: Some(launched),
            title: String::new(),
        }
    }

    #[must_use]
    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus_handle
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        match keys::route(keystroke, self.document.mode()) {
            Route::Engine(key) => {
                cx.stop_propagation();
                self.marked = None;
                let outcome = self.document.feed(key, &mut SystemClipboard(cx));
                self.handle_outcome(outcome, window, cx);
            }
            Route::TextInput => {}
            Route::Ignored => log::debug!("tecla sin equivalente en nota-vim: {keystroke}"),
        }
    }

    /// Texto que llega por el input handler: teclado compuesto o IME.
    fn commit_text(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let marked_range = self
            .ime_line()
            .filter(|_| self.marked.is_some())
            .map(|ime| ime.marked_range());
        self.marked = None;
        if let Some(range) = range.filter(|range| Some(range) != marked_range.as_ref()) {
            log::warn!("el IME pidió reemplazar {range:?}; solo se escribe en el cursor");
        }
        let outcome = self
            .document
            .type_text(text, &mut SystemClipboard(cx))
            .transpose();
        match outcome {
            Some(outcome) => self.handle_outcome(outcome, window, cx),
            None => cx.notify(),
        }
    }

    fn set_preedit(&mut self, preedit: Option<String>, cx: &mut Context<Self>) {
        self.marked = preedit;
        cx.notify();
    }

    fn handle_outcome(
        &mut self,
        outcome: Result<KeyOutcome, DocumentError>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match outcome {
            Ok(KeyOutcome::Executed(Action::CommandLine)) => self.message = None,
            Ok(KeyOutcome::Command(command)) => {
                window.dispatch_action(actions::for_command(command), cx);
            }
            Ok(KeyOutcome::InvalidCommand(error)) => {
                self.message = Some(Message::Error(error.to_string()));
            }
            Ok(_) => {}
            Err(error) => self.report_error(&error),
        }
        self.reveal_cursor(cx);
    }

    /// La línea donde se escribe, tal como la ve el IME.
    fn ime_line(&self) -> Option<ImeLine> {
        let marked = self.marked.as_deref().unwrap_or("");
        if let Some(typed) = self.document.command_line() {
            return Some(ImeLine::new(typed.to_owned(), typed.len(), marked));
        }
        let buffer = self.document.buffer();
        let cursor = self.document.cursor();
        let line = buffer.line_of(cursor);
        let text = self.document.line_text(line).ok()?;
        let column = cursor.get() - buffer.line_start(line).get();
        Some(ImeLine::new(text, column, marked))
    }

    fn reveal_cursor(&mut self, cx: &mut Context<Self>) {
        if let Some(layout) = &self.layout {
            let line = self.document.buffer().line_of(self.document.cursor());
            self.scroll.reveal(line, layout.viewport.full_rows());
        }
        cx.notify();
    }

    fn report_error(&mut self, error: &DocumentError) {
        log::error!("{error}");
        self.message = Some(Message::Error(error.to_string()));
    }

    fn report_first_paint(&mut self) {
        if let Some(launched) = self.launched.take() {
            log::info!(
                "primer frame a {} ms del arranque",
                launched.elapsed().as_millis()
            );
        }
    }

    fn status(&self) -> Status {
        let buffer = self.document.buffer();
        let cursor = self.document.cursor();
        let line = buffer.line_of(cursor);
        let command_line = self
            .document
            .command_line()
            .map(|typed| (typed.to_owned(), self.marked.clone().unwrap_or_default()));
        Status {
            mode: self.document.mode(),
            recording: self.document.recording(),
            file: self.file_name(),
            modified: self.document.is_modified(),
            command_line,
            message: self.message.clone(),
            position: (line + 1, buffer.column_of(cursor) + 1),
        }
    }

    fn file_name(&self) -> SharedString {
        self.document.path().display().to_string().into()
    }

    fn update_title(&mut self, window: &mut Window) {
        let modified = if self.document.is_modified() {
            " [+]"
        } else {
            ""
        };
        let title = format!("{}{modified} — nota", self.file_name());
        if title != self.title {
            window.set_window_title(&title);
            self.title = title;
        }
    }
}

/// Como vim al abrir: `"a.md" 3L, 20B`, o `[Nuevo]` si todavía no existe.
fn opened_message(document: &Document) -> Message {
    let path = document.path().display();
    if document.exists_on_disk() {
        Message::Info(format!("\"{path}\" {}", document.stats()))
    } else {
        Message::Info(format!("\"{path}\" [Nuevo]"))
    }
}

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.update_title(window);
        let theme = Theme::get(cx).clone();
        let status = status_bar(self.status(), &theme);
        div()
            .key_context(EDITOR_CONTEXT)
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .font_family(theme.font_family.clone())
            .text_size(theme.font_size)
            .line_height(relative(theme.line_height))
            .text_color(theme.foreground)
            .bg(theme.background)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_action(cx.listener(Self::save))
            .on_action(cx.listener(Self::quit))
            .on_action(commands::force_quit)
            .on_action(cx.listener(Self::save_and_quit))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::paste))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .cursor(CursorStyle::IBeam)
                    .on_scroll_wheel(cx.listener(Self::on_scroll))
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::on_left_down))
                    .on_mouse_move(cx.listener(Self::on_mouse_move))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::on_left_up))
                    .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_left_up))
                    .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_middle_down))
                    .child(EditorElement::new(cx.entity(), theme.clone())),
            )
            .child(status)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn el_mensaje_al_abrir_dice_si_es_nuevo() {
        let document = Document::new(Path::new("a.md"), "");
        assert_eq!(
            opened_message(&document),
            Message::Info("\"a.md\" [Nuevo]".to_owned())
        );
    }
}
