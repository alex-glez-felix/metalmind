//! El texto que se escribe en el panel: un nombre o el filtro. Enter y
//! Escape son acciones del registro ([`metalmind_keymap::PanelAction`]);
//! lo demás llega aquí.

use gpui_kit::{Context, KeyDownEvent, Keystroke, Window};

use super::Sidebar;
use crate::picker::{InputLine, LineEdit};

impl Sidebar {
    pub(super) fn on_key_down(
        &mut self,
        event: &KeyDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(input) = self.panel.input_mut() else {
            return;
        };
        if edit(input, &event.keystroke) {
            self.panel.input_changed();
            self.scroll_to_selection();
            cx.stop_propagation();
            cx.notify();
        }
    }
}

/// Aplica `keystroke` a `input`; devuelve si era una tecla del campo.
fn edit(input: &mut InputLine, keystroke: &Keystroke) -> bool {
    let Some(edit) = LineEdit::for_key(keystroke).or_else(|| typed(keystroke)) else {
        return false;
    };
    input.apply(edit);
    true
}

/// El texto que escribe `keystroke`, si no lleva modificadores de atajo.
fn typed(keystroke: &Keystroke) -> Option<LineEdit<'_>> {
    let modifiers = keystroke.modifiers;
    if modifiers.control || modifiers.alt || modifiers.platform || modifiers.function {
        return None;
    }
    keystroke.key_char.as_deref().map(LineEdit::Insert)
}

#[cfg(test)]
mod tests {
    use gpui_kit::Modifiers;

    use super::*;

    fn stroke(key: &str, key_char: Option<&str>, modifiers: Modifiers) -> Keystroke {
        Keystroke {
            modifiers,
            key: key.to_owned(),
            key_char: key_char.map(str::to_owned),
        }
    }

    #[test]
    fn escribe_borra_y_se_mueve() {
        let mut input = InputLine::new("nota");
        let none = Modifiers::none();
        assert!(edit(&mut input, &stroke("a", Some("a"), none)));
        assert!(edit(&mut input, &stroke("aacute", Some("á"), none)));
        assert!(edit(&mut input, &stroke("left", None, none)));
        assert!(edit(&mut input, &stroke("backspace", None, none)));
        assert_eq!(input.text(), "notaá");
        assert!(edit(&mut input, &stroke("u", None, Modifiers::control())));
        assert_eq!(input.text(), "á");
        assert!(!edit(&mut input, &stroke("x", Some("x"), Modifiers::alt())));
        assert!(!edit(&mut input, &stroke("f1", None, none)));
        assert!(!edit(&mut input, &stroke("c", None, Modifiers::control())));
        assert_eq!(input.text(), "á");
    }

    /// Las preguntas del panel editan como el input del picker: por grafema
    /// y con `ctrl-w`.
    #[test]
    fn borra_por_grafema_y_por_palabra() {
        let none = Modifiers::none();
        let mut input = InputLine::new("cafe\u{301}");
        assert!(edit(&mut input, &stroke("backspace", None, none)));
        assert_eq!(input.text(), "caf", "el acento combinante se va con su e");
        let mut input = InputLine::new("diario/hoy  nota");
        assert!(edit(&mut input, &stroke("w", None, Modifiers::control())));
        assert_eq!(input.text(), "diario/hoy  ");
        assert!(edit(
            &mut input,
            &stroke("backspace", None, Modifiers::control())
        ));
        assert_eq!(input.text(), "");
    }
}
