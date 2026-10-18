use super::SystemRegister;

/// De qué registro sale o a cuál va lo que se copia, borra o pega.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RegisterName {
    /// `""`.
    Unnamed,
    /// `"a`–`"z`, por su índice.
    Named(u8),
    /// `"A`–`"Z`: escribir agrega al final de `"a`–`"z`.
    Append(u8),
    /// `"0`: el último yank sin registro.
    Yank,
    /// `"1`–`"9`: los últimos borrados de líneas.
    Numbered(u8),
    /// `"-`: el último borrado dentro de una línea.
    SmallDelete,
    /// `"_`: lo que se escribe ahí se pierde.
    BlackHole,
    /// `".`: lo último insertado. Solo lectura.
    LastInsert,
    /// `"/`: la última búsqueda. Solo lectura.
    LastSearch,
    /// `"+` y `"*`.
    System(SystemRegister),
}

impl RegisterName {
    pub fn parse(c: char) -> Option<Self> {
        let name = match c {
            '"' => Self::Unnamed,
            'a'..='z' => Self::Named(letter_index(c)),
            'A'..='Z' => Self::Append(letter_index(c.to_ascii_lowercase())),
            '0' => Self::Yank,
            '1'..='9' => Self::Numbered(c as u8 - b'0'),
            '-' => Self::SmallDelete,
            '_' => Self::BlackHole,
            '.' => Self::LastInsert,
            '/' => Self::LastSearch,
            '+' => Self::System(SystemRegister::Clipboard),
            '*' => Self::System(SystemRegister::Primary),
            _ => return None,
        };
        Some(name)
    }

    /// `"a`–`"z` o `"A`–`"Z`: el índice de la letra y si agrega.
    pub fn letter(self) -> Option<(u8, bool)> {
        match self {
            Self::Named(letter) => Some((letter, false)),
            Self::Append(letter) => Some((letter, true)),
            _ => None,
        }
    }

    pub fn is_writable(self) -> bool {
        !matches!(self, Self::LastInsert | Self::LastSearch)
    }

    pub fn appends(self) -> bool {
        matches!(self, Self::Append(_))
    }

    /// Como en vim, `.` tras `"1p` pega `"2`, y así hasta `"9`.
    pub fn next_for_repeat(self) -> Self {
        match self {
            Self::Numbered(n) if n < 9 => Self::Numbered(n + 1),
            other => other,
        }
    }
}

fn letter_index(c: char) -> u8 {
    c as u8 - b'a'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_register_vim_has() {
        assert_eq!(RegisterName::parse('c'), Some(RegisterName::Named(2)));
        assert_eq!(RegisterName::parse('C'), Some(RegisterName::Append(2)));
        assert_eq!(RegisterName::parse('3'), Some(RegisterName::Numbered(3)));
        assert_eq!(
            RegisterName::parse('*'),
            Some(RegisterName::System(SystemRegister::Primary))
        );
        assert_eq!(RegisterName::parse('!'), None);
        assert_eq!(RegisterName::parse('ñ'), None);
    }

    #[test]
    fn repeating_walks_the_numbered_registers_up_to_nine() {
        let next = |n| RegisterName::Numbered(n).next_for_repeat();
        assert_eq!(next(1), RegisterName::Numbered(2));
        assert_eq!(next(9), RegisterName::Numbered(9));
        assert_eq!(
            RegisterName::Named(0).next_for_repeat(),
            RegisterName::Named(0)
        );
    }
}
