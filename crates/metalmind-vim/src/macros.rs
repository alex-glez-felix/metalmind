//! Las teclas de una macro como texto de registro, codificadas como las
//! guarda vim: `<Esc>` es `^[`, `<CR>` es `^M`, `ctrl-r` es `^R`, y las
//! que no tienen carácter van tras `K_SPECIAL` (`<BS>` es `<80>kb`).

use crate::Key;
use crate::register::Register;

const ESCAPE: char = '\u{1b}';
/// `ctrl-h`, que en Insert borra como `<BS>`.
const CTRL_H: char = '\u{8}';
/// El `K_SPECIAL` de vim, seguido del nombre de la tecla.
const SPECIAL: char = '\u{80}';

const SPECIAL_KEYS: [(Key, &str); 4] = [
    (Key::Backspace, "kb"),
    (Key::ShiftTab, "kB"),
    (Key::Up, "ku"),
    (Key::Down, "kd"),
];

pub(crate) fn encode(keys: &[Key]) -> String {
    let mut text = String::new();
    for &key in keys {
        encode_key(key, &mut text);
    }
    text
}

/// Lo que ejecuta `@`: como en vim, cada línea de un registro por líneas
/// termina en `\n`, que en Normal es `ctrl-j`; en uno por bloque, todas
/// menos la última.
pub(crate) fn decode(register: &Register) -> Vec<Key> {
    let mut text = register.text.clone();
    if register.is_linewise() {
        text.push('\n');
    }
    let mut keys = Vec::new();
    let mut rest = text.as_str();
    while let Some(c) = rest.chars().next() {
        rest = &rest[c.len_utf8()..];
        let special = (c == SPECIAL)
            .then(|| SPECIAL_KEYS.iter().find(|(_, name)| rest.starts_with(name)))
            .flatten();
        match special {
            Some(&(key, name)) => {
                rest = &rest[name.len()..];
                keys.push(key);
            }
            None => keys.push(decode_char(c)),
        }
    }
    keys
}

/// Un `ctrl` sin carácter de control no se graba: vim tampoco lo guarda así.
fn encode_key(key: Key, text: &mut String) {
    let c = match key {
        Key::Char(c) => Some(c),
        Key::Ctrl(c) => control_char(c),
        Key::Escape => Some(ESCAPE),
        Key::Enter => Some('\r'),
        Key::Tab => Some('\t'),
        Key::Backspace | Key::ShiftTab | Key::Up | Key::Down => None,
    };
    if let Some(c) = c {
        text.push(c);
    } else if let Some((_, name)) = SPECIAL_KEYS.iter().find(|(special, _)| *special == key) {
        text.push(SPECIAL);
        text.push_str(name);
    }
}

fn control_char(c: char) -> Option<char> {
    let upper = c.to_ascii_uppercase();
    ('@'..='_')
        .contains(&upper)
        .then(|| char::from(upper as u8 ^ 0x40))
}

fn decode_char(c: char) -> Key {
    match c {
        ESCAPE => Key::Escape,
        '\r' => Key::Enter,
        CTRL_H => Key::Backspace,
        '\t' => Key::Tab,
        '\u{0}'..='\u{1f}' => Key::Ctrl(char::from(c as u8 ^ 0x40).to_ascii_lowercase()),
        _ => Key::Char(c),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(text: &str) -> Register {
        Register::chars(text)
    }

    #[test]
    fn special_keys_round_trip_through_the_register() {
        let keys = [
            Key::Char('i'),
            Key::Char('ñ'),
            Key::Escape,
            Key::Enter,
            Key::Backspace,
            Key::Tab,
            Key::ShiftTab,
            Key::Up,
            Key::Down,
            Key::Ctrl('r'),
            Key::Ctrl('j'),
        ];
        let encoded = encode(&keys);
        assert_eq!(
            encoded,
            "iñ\u{1b}\r\u{80}kb\t\u{80}kB\u{80}ku\u{80}kd\u{12}\n"
        );
        assert_eq!(decode(&chars(&encoded)), keys);
    }

    #[test]
    fn ctrl_h_and_a_lone_special_byte_decode_as_typed() {
        assert_eq!(
            decode(&chars("\u{8}\u{80}x")),
            [Key::Backspace, Key::Char('\u{80}'), Key::Char('x')]
        );
    }

    #[test]
    fn a_ctrl_without_a_control_char_is_not_recorded() {
        assert_eq!(encode(&[Key::Ctrl('1'), Key::Char('x')]), "x");
    }

    #[test]
    fn each_line_of_a_linewise_register_ends_in_ctrl_j() {
        let register = Register::lines("x\ny");
        let ctrl_j = Key::Ctrl('j');
        assert_eq!(
            decode(&register),
            [Key::Char('x'), ctrl_j, Key::Char('y'), ctrl_j]
        );
    }
}
