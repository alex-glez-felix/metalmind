//! `:focus` y `:unfocus` en la ventana: el focus vive en la sesión del
//! vault, que lo comparten el finder, la búsqueda, el panel y las barras de
//! estado (05 §3).

use gpui_kit::{Context, Window};

use super::WorkspaceView;
use crate::focus::Focus;
use crate::workspace::Message;

impl WorkspaceView {
    /// `:focus`: con alcance lo pone y sin él dice el de ahora (05 §3).
    pub(super) fn run_focus(
        &mut self,
        argument: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match written_scope(argument) {
            Some(text) => self.focus_on(text, window, cx),
            None => self.show_focus(window, cx),
        }
    }

    /// `:focus` con un alcance, que puede nombrar vistas guardadas (`@gs`).
    fn focus_on(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
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
    fn show_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let shown = self
            .session
            .read(cx)
            .focus()
            .map_or_else(|| "sin focus".to_owned(), Focus::label);
        self.tell(Message::Info(shown), cx);
        self.focus(window, cx);
    }
}

/// Un alcance en blanco es como no escribirlo.
fn written_scope(argument: Option<&str>) -> Option<&str> {
    argument.filter(|text| !text.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::written_scope;

    #[test]
    fn un_alcance_en_blanco_es_como_no_escribirlo() {
        assert_eq!(written_scope(None), None);
        assert_eq!(written_scope(Some("   ")), None);
        assert_eq!(
            written_scope(Some("proyectos/   @gs")),
            Some("proyectos/   @gs")
        );
    }
}
