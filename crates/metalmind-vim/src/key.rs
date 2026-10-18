/// Una tecla ya interpretada por la capa de entrada.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Char(char),
    Ctrl(char),
    Escape,
    Enter,
    Backspace,
    Tab,
    Up,
    Down,
}

/// Como escribe las teclas GPUI (`ctrl-w`, `space`), que es como se ven en
/// la paleta junto a los atajos de la ventana.
impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Char(' ') => f.write_str("space"),
            Self::Char(c) => write!(f, "{c}"),
            Self::Ctrl(c) => write!(f, "ctrl-{c}"),
            Self::Escape => f.write_str("escape"),
            Self::Enter => f.write_str("enter"),
            Self::Backspace => f.write_str("backspace"),
            Self::Tab => f.write_str("tab"),
            Self::Up => f.write_str("up"),
            Self::Down => f.write_str("down"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Key;

    #[test]
    fn se_escriben_como_los_atajos_de_gpui() {
        let cases = [
            (Key::Char('g'), "g"),
            (Key::Char(' '), "space"),
            (Key::Char('G'), "G"),
            (Key::Ctrl('w'), "ctrl-w"),
            (Key::Escape, "escape"),
            (Key::Enter, "enter"),
        ];
        for (key, text) in cases {
            assert_eq!(key.to_string(), text);
        }
    }
}
