//! Borde entre las teclas de GPUI y las [`Key`] de `nota-vim`.
//!
//! En Linux GPUI entrega el texto compuesto (teclas muertas, `AltGr`) en
//! `key_char`, y solo manda al input handler los `key_char` sin más
//! modificador que shift; las commits multibyte de un IME llegan directo al
//! input handler.

use gpui_kit::Keystroke;
use nota_vim::{Key, Mode};

/// Por dónde debe entrar una tecla al motor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// Se manda ya a `Vim::feed` y no sigue propagándose.
    Engine(Key),
    /// Texto en Insert o en la línea de comandos: se deja pasar para que GPUI
    /// lo entregue por el input handler, el mismo camino que usan las teclas
    /// muertas y el IME.
    TextInput,
    /// No tiene equivalente en `nota-vim`.
    Ignored,
}

pub fn translate(keystroke: &Keystroke) -> Option<Key> {
    let modifiers = keystroke.modifiers;
    if modifiers.alt || modifiers.platform || modifiers.function {
        return None;
    }
    if modifiers.control {
        return control_key(keystroke);
    }
    if !modifiers.shift
        && let Some(key) = named_key(&keystroke.key)
    {
        return Some(key);
    }
    single_char(keystroke.key_char.as_deref()?).map(Key::Char)
}

pub fn route(keystroke: &Keystroke, mode: Mode) -> Route {
    match translate(keystroke) {
        None => Route::Ignored,
        Some(Key::Char(_)) if mode.takes_text() => Route::TextInput,
        Some(key) => Route::Engine(key),
    }
}

/// `ctrl-shift-*` queda para los atajos de la aplicación.
fn control_key(keystroke: &Keystroke) -> Option<Key> {
    if keystroke.modifiers.shift {
        return None;
    }
    single_char(&keystroke.key)
        .filter(char::is_ascii_graphic)
        .map(Key::Ctrl)
}

fn named_key(name: &str) -> Option<Key> {
    match name {
        "escape" => Some(Key::Escape),
        "enter" => Some(Key::Enter),
        "backspace" => Some(Key::Backspace),
        "tab" => Some(Key::Tab),
        _ => None,
    }
}

fn single_char(text: &str) -> Option<char> {
    let mut chars = text.chars();
    let first = chars.next()?;
    chars.next().is_none().then_some(first)
}

#[cfg(test)]
mod tests {
    use gpui_kit::Modifiers;

    use super::*;

    #[derive(Clone, Copy)]
    enum Mods {
        None,
        Shift,
        Ctrl,
        CtrlShift,
        Alt,
        Super,
    }

    /// Construye el `Keystroke` tal como lo arma `gpui_linux` con xkb.
    fn stroke(key: &str, key_char: Option<&str>, mods: Mods) -> Keystroke {
        let modifiers = match mods {
            Mods::None => Modifiers::none(),
            Mods::Shift => Modifiers::shift(),
            Mods::Ctrl => Modifiers::control(),
            Mods::CtrlShift => Modifiers::control_shift(),
            Mods::Alt => Modifiers::alt(),
            Mods::Super => Modifiers::command(),
        };
        Keystroke {
            modifiers,
            key: key.to_owned(),
            key_char: key_char.map(str::to_owned),
        }
    }

    #[test]
    fn traduccion() {
        let cases = [
            ("a", Some("a"), Mods::None, Some(Key::Char('a'))),
            ("a", Some("A"), Mods::Shift, Some(Key::Char('A'))),
            ("4", Some("$"), Mods::None, Some(Key::Char('$'))),
            ("space", Some(" "), Mods::None, Some(Key::Char(' '))),
            // ´ + a en layout latam: xkb compone y GPUI pone el resultado en key_char.
            ("aacute", Some("á"), Mods::None, Some(Key::Char('á'))),
            ("ntilde", Some("ñ"), Mods::None, Some(Key::Char('ñ'))),
            // La tecla muerta sola no produce texto: el preedit va por el IME.
            ("[", None, Mods::None, None),
            // AltGr no se reporta como alt.
            ("q", Some("@"), Mods::None, Some(Key::Char('@'))),
            ("escape", None, Mods::None, Some(Key::Escape)),
            ("enter", None, Mods::None, Some(Key::Enter)),
            ("backspace", None, Mods::None, Some(Key::Backspace)),
            ("tab", None, Mods::None, Some(Key::Tab)),
            ("tab", None, Mods::Shift, None),
            ("r", None, Mods::Ctrl, Some(Key::Ctrl('r'))),
            ("[", None, Mods::Ctrl, Some(Key::Ctrl('['))),
            ("c", None, Mods::CtrlShift, None),
            ("x", Some("x"), Mods::Alt, None),
            ("a", Some("a"), Mods::Super, None),
            ("left", None, Mods::None, None),
            ("f1", None, Mods::None, None),
        ];
        for (key, key_char, mods, expected) in cases {
            assert_eq!(
                translate(&stroke(key, key_char, mods)),
                expected,
                "key={key:?} key_char={key_char:?}"
            );
        }
    }

    #[test]
    fn ruta_segun_modo() {
        let cases = [
            (
                "a",
                Some("a"),
                Mods::None,
                Mode::Normal,
                Route::Engine(Key::Char('a')),
            ),
            ("a", Some("a"), Mods::None, Mode::Insert, Route::TextInput),
            (
                "w",
                Some("w"),
                Mods::None,
                Mode::CommandLine,
                Route::TextInput,
            ),
            (
                "enter",
                None,
                Mods::None,
                Mode::CommandLine,
                Route::Engine(Key::Enter),
            ),
            (
                "aacute",
                Some("á"),
                Mods::None,
                Mode::Insert,
                Route::TextInput,
            ),
            (
                "escape",
                None,
                Mods::None,
                Mode::Insert,
                Route::Engine(Key::Escape),
            ),
            (
                "enter",
                None,
                Mods::None,
                Mode::Insert,
                Route::Engine(Key::Enter),
            ),
            (
                "w",
                None,
                Mods::Ctrl,
                Mode::Insert,
                Route::Engine(Key::Ctrl('w')),
            ),
            (
                "c",
                None,
                Mods::Ctrl,
                Mode::Insert,
                Route::Engine(Key::Ctrl('c')),
            ),
            ("[", None, Mods::None, Mode::Insert, Route::Ignored),
            ("[", None, Mods::None, Mode::Normal, Route::Ignored),
        ];
        for (key, key_char, mods, mode, expected) in cases {
            assert_eq!(
                route(&stroke(key, key_char, mods), mode),
                expected,
                "key={key:?} mode={mode:?}"
            );
        }
    }
}
