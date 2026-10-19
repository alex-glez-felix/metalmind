//! El texto que se escribe en el panel: un nombre o el filtro. Enter y
//! Escape son acciones del registro ([`metalmind_keymap::PanelAction`]);
//! lo demás llega aquí.

use gpui_kit::{Context, KeyDownEvent, Keystroke, Window};

use super::Sidebar;
use crate::panel::LineInput;

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
fn edit(input: &mut LineInput, keystroke: &Keystroke) -> bool {
    let modifiers = keystroke.modifiers;
    if modifiers.alt || modifiers.platform || modifiers.function {
        return false;
    }
    if modifiers.control {
        return match keystroke.key.as_str() {
            "u" => {
                input.clear_before();
                true
            }
            _ => false,
        };
    }
    match keystroke.key.as_str() {
        "backspace" => input.backspace(),
        "delete" => input.delete(),
        "left" => input.left(),
        "right" => input.right(),
        "home" => input.home(),
        "end" => input.end(),
        _ => match keystroke.key_char.as_deref() {
            Some(text) => input.insert(text),
            None => return false,
        },
    }
    true
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
        let mut input = LineInput::new("nota");
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
}
