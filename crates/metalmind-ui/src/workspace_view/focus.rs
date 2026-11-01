//! `:focus` y `:unfocus` en la ventana: el focus vive en la sesión del
//! vault, que lo comparten el finder, la búsqueda, el panel y las barras de
//! estado (05 §3).

use gpui_kit::{Context, Window};

use super::WorkspaceView;
use crate::focus::Focus;
use crate::workspace::Message;

impl WorkspaceView {
    /// `:focus` con un alcance, que puede nombrar vistas guardadas (`@gs`).
    pub(super) fn focus_on(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let views = self.session.read(cx).views();
        match Focus::parse(text, &views) {
            Ok(focus) => self.set_focus(focus, window, cx),
            Err(error) => {
                self.tell(Message::Error(error.to_string()), cx);
                self.focus(window, cx);
            }
        }
    }

    pub(super) fn set_focus(
        &mut self,
        focus: Option<Focus>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.session
            .update(cx, |session, cx| session.set_focus(focus, cx));
        self.focus(window, cx);
    }

    /// `:focus` sin alcance: el de ahora, en la línea de mensajes.
    pub(super) fn show_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let shown = self
            .session
            .read(cx)
            .focus()
            .map_or_else(|| "sin focus".to_owned(), Focus::label);
        self.tell(Message::Info(shown), cx);
        self.focus(window, cx);
    }
}
