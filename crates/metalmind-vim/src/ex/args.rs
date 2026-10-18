//! Lo que va tras el nombre de `:d`, `:y`, `:j`, `:>` y `:<`: un registro
//! y un count.

use super::address::{leading_number, skip_blanks};
use crate::CommandError;
use crate::register::RegisterName;

/// `[x] [count]` de `:d` y `:y`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct RegisterAndCount {
    pub register: Option<RegisterName>,
    pub count: Option<usize>,
}

impl RegisterAndCount {
    /// Como vim, un dígito es el count y no el registro.
    ///
    /// # Errors
    /// Con un count 0 o texto de más.
    pub fn parse(args: &str) -> Result<Self, CommandError> {
        let args = skip_blanks(args);
        let mut chars = args.chars();
        let register = chars
            .next()
            .filter(|c| !c.is_ascii_digit())
            .and_then(RegisterName::parse)
            .filter(|name| name.is_writable());
        let rest = if register.is_some() {
            chars.as_str()
        } else {
            args
        };
        Ok(Self {
            register,
            count: count(rest)?,
        })
    }
}

/// `[count]`: con count, el comando va desde la última línea del rango y
/// abarca `count` líneas.
///
/// # Errors
/// Con un count 0 o texto de más.
pub(crate) fn count(args: &str) -> Result<Option<usize>, CommandError> {
    let (count, rest) = leading_number(skip_blanks(args))?;
    if count == Some(0) {
        return Err(CommandError::PositiveCountRequired);
    }
    no_trailing(rest)?;
    Ok(count)
}

/// # Errors
/// [`CommandError::TrailingCharacters`] si queda algo más que espacios.
pub(crate) fn no_trailing(rest: &str) -> Result<(), CommandError> {
    let rest = rest.trim();
    if rest.is_empty() {
        Ok(())
    } else {
        Err(CommandError::TrailingCharacters(rest.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(register: Option<char>, count: Option<usize>) -> RegisterAndCount {
        RegisterAndCount {
            register: register.and_then(RegisterName::parse),
            count,
        }
    }

    #[test]
    fn reads_the_register_and_the_count() {
        let cases = [
            ("", parsed(None, None)),
            (" a", parsed(Some('a'), None)),
            ("A 3", parsed(Some('A'), Some(3))),
            ("_2", parsed(Some('_'), Some(2))),
            (" 12 ", parsed(None, Some(12))),
            ("+", parsed(Some('+'), None)),
        ];
        for (args, expected) in cases {
            assert_eq!(RegisterAndCount::parse(args), Ok(expected), "{args:?}");
        }
    }

    #[test]
    fn rejects_a_zero_count_and_leftovers() {
        assert_eq!(
            RegisterAndCount::parse("a 0"),
            Err(CommandError::PositiveCountRequired)
        );
        assert_eq!(
            RegisterAndCount::parse(". 2"),
            Err(CommandError::TrailingCharacters(". 2".to_owned()))
        );
        assert_eq!(
            count("2 x"),
            Err(CommandError::TrailingCharacters("x".to_owned()))
        );
    }
}
