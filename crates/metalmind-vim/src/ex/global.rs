//! `:g/patrón/comando`, `:g!` y `:v`, tal como se escribieron.

use std::fmt;

use super::address::skip_blanks;
use crate::CommandError;
use crate::search::split_at_delimiter;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GlobalCommand {
    /// Vacío es el último patrón usado.
    pub pattern: String,
    /// `:g!` o `:v`: las líneas donde el patrón no está.
    pub inverted: bool,
    /// Lo que se corre en cada línea; vacío solo va a ella, como el `:p` de
    /// vim sin imprimir.
    pub command: String,
}

/// Un `:g` que no encontró líneas en las que correr: vim lo avisa sin que
/// sea un error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlobalMiss {
    pub pattern: String,
    /// `:g!` o `:v`: el patrón estaba en todas.
    pub inverted: bool,
}

impl fmt::Display for GlobalMiss {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.inverted {
            write!(f, "Pattern found in every line: {}", self.pattern)
        } else {
            write!(f, "Pattern not found: {}", self.pattern)
        }
    }
}

impl GlobalCommand {
    /// Como vim, el delimitador es cualquier carácter que no sea letra.
    ///
    /// # Errors
    /// Sin patrón o con una letra de delimitador.
    pub fn parse(args: &str, inverted: bool) -> Result<Self, CommandError> {
        let args = skip_blanks(args);
        let delimiter = args
            .chars()
            .next()
            .ok_or(CommandError::GlobalPatternMissing)?;
        if delimiter.is_ascii_alphabetic() {
            return Err(CommandError::LetterDelimiter);
        }
        let (pattern, rest) = split_at_delimiter(&args[delimiter.len_utf8()..], delimiter);
        Ok(Self {
            pattern,
            inverted,
            command: rest.unwrap_or_default().to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn global(pattern: &str, command: &str) -> GlobalCommand {
        GlobalCommand {
            pattern: pattern.to_owned(),
            inverted: false,
            command: command.to_owned(),
        }
    }

    #[test]
    fn splits_pattern_and_command() {
        let cases = [
            ("/a/d", global("a", "d")),
            (" #a/b#s/x/y/", global("a/b", "s/x/y/")),
            ("/a\\/b/normal Ax", global("a\\/b", "normal Ax")),
            ("/a", global("a", "")),
            ("//m0", global("", "m0")),
        ];
        for (args, expected) in cases {
            assert_eq!(GlobalCommand::parse(args, false), Ok(expected), "{args:?}");
        }
    }

    #[test]
    fn needs_a_pattern_not_delimited_by_a_letter() {
        assert_eq!(
            GlobalCommand::parse("", false),
            Err(CommandError::GlobalPatternMissing)
        );
        assert_eq!(
            GlobalCommand::parse("xax", false),
            Err(CommandError::LetterDelimiter)
        );
    }
}
