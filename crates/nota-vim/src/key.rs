use std::fmt;
use std::str::FromStr;

use crate::KeymapError;

/// Una tecla ya interpretada por la capa de entrada.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Char(char),
    Ctrl(char),
    Escape,
    Enter,
    Backspace,
    Tab,
}

impl Key {
    /// Parsea una secuencia separada por espacios, p. ej. `"g g"` o `"ctrl-w h"`.
    ///
    /// # Errors
    /// Si la secuencia está vacía o alguna tecla no se reconoce.
    pub fn parse_sequence(sequence: &str) -> Result<Vec<Self>, KeymapError> {
        let keys = sequence
            .split_whitespace()
            .map(str::parse)
            .collect::<Result<Vec<_>, _>>()?;
        if keys.is_empty() {
            return Err(KeymapError::EmptySequence);
        }
        Ok(keys)
    }
}

impl FromStr for Key {
    type Err = KeymapError;

    /// Notación de Zed: `w`, `W`, `$`, `space`, `escape`, `ctrl-r`.
    fn from_str(token: &str) -> Result<Self, Self::Err> {
        let unknown = || KeymapError::UnknownKey(token.to_owned());
        let key = match token {
            "escape" => Self::Escape,
            "enter" => Self::Enter,
            "backspace" => Self::Backspace,
            "tab" => Self::Tab,
            "space" => Self::Char(' '),
            _ => match token.strip_prefix("ctrl-") {
                Some(rest) => Self::Ctrl(single_char(rest).ok_or_else(unknown)?),
                None => Self::Char(single_char(token).ok_or_else(unknown)?),
            },
        };
        Ok(key)
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Char(' ') => f.write_str("space"),
            Self::Char(c) => write!(f, "{c}"),
            Self::Ctrl(c) => write!(f, "ctrl-{c}"),
            Self::Escape => f.write_str("escape"),
            Self::Enter => f.write_str("enter"),
            Self::Backspace => f.write_str("backspace"),
            Self::Tab => f.write_str("tab"),
        }
    }
}

fn single_char(token: &str) -> Option<char> {
    let mut chars = token.chars();
    let c = chars.next()?;
    chars.next().is_none().then_some(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_zed_notation() {
        let keys = Key::parse_sequence("g g ctrl-r escape space $").unwrap();
        assert_eq!(
            keys,
            [
                Key::Char('g'),
                Key::Char('g'),
                Key::Ctrl('r'),
                Key::Escape,
                Key::Char(' '),
                Key::Char('$'),
            ]
        );
    }

    #[test]
    fn display_round_trips() {
        for token in ["w", "ctrl-r", "escape", "space", "enter"] {
            assert_eq!(token.parse::<Key>().unwrap().to_string(), token);
        }
    }

    #[test]
    fn rejects_unknown_keys() {
        assert_eq!(
            "ctrl-".parse::<Key>(),
            Err(KeymapError::UnknownKey("ctrl-".into()))
        );
        assert_eq!(Key::parse_sequence("  "), Err(KeymapError::EmptySequence));
    }
}
