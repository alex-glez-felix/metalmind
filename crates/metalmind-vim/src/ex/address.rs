//! Rangos de líneas de `:` tal como se escribieron: `%`, `.`, `$`, `N`,
//! `'a`, `/patrón/` y `?patrón?`, con desplazamientos (`.-1`, `/x/+2`),
//! separados por `,` o por `;`, que lleva el cursor a la dirección anterior.

use crate::CommandError;
use crate::search::split_at_delimiter;

/// Un rango antes de saber dónde está el cursor ni qué dicen las marks y
/// los patrones.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LineRange {
    /// `%`
    Whole,
    /// En orden; como en vim, cuentan las dos últimas.
    Addresses(Vec<Step>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Step {
    /// `None` si se omitió, como en `,5`: es la línea del cursor.
    pub address: Option<Address>,
    /// Le sigue un `;`: el cursor pasa a ella antes de la siguiente.
    pub moves_cursor: bool,
}

/// Una base con su desplazamiento, y quizá otras búsquedas que empiezan
/// donde quedó la anterior (`/a//b/`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Address {
    pub terms: Vec<Term>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Term {
    pub base: Base,
    pub offset: isize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Base {
    /// `.`, o nada antes de un desplazamiento (`+2`).
    Current,
    /// `$`
    Last,
    /// Desde 1, como se escribe; `0` es antes de la primera.
    Number(usize),
    /// `'x`
    Mark(char),
    /// `/patrón/` o `?patrón?`; vacío es el último patrón.
    Search { pattern: String, backward: bool },
}

impl LineRange {
    /// El rango al inicio de `text`, si hay, y lo que sigue.
    ///
    /// # Errors
    /// [`CommandError::InvalidRange`] si un número no cabe.
    pub fn parse(text: &str) -> Result<(Option<Self>, &str), CommandError> {
        let text = skip_blanks(text);
        if let Some(rest) = text.strip_prefix('%') {
            return Ok((Some(Self::Whole), rest));
        }
        let (mut address, mut rest) = Address::parse(text)?;
        let mut steps = Vec::new();
        loop {
            let after = skip_blanks(rest);
            let Some(separator) = after.chars().next().filter(|&c| c == ',' || c == ';') else {
                if steps.is_empty() && address.is_none() {
                    return Ok((None, text));
                }
                steps.push(Step {
                    address,
                    moves_cursor: false,
                });
                return Ok((Some(Self::Addresses(steps)), after));
            };
            steps.push(Step {
                address,
                moves_cursor: separator == ';',
            });
            (address, rest) = Address::parse(&after[1..])?;
        }
    }
}

impl Address {
    /// La dirección al inicio de `text`, si hay, y lo que sigue.
    ///
    /// # Errors
    /// [`CommandError::InvalidRange`] si un número no cabe.
    pub fn parse(text: &str) -> Result<(Option<Self>, &str), CommandError> {
        let mut terms = Vec::new();
        let mut rest = skip_blanks(text);
        loop {
            let (base, after) = Base::parse(rest)?;
            let (offset, after) = offset(after)?;
            if base.is_none() && offset.is_none() {
                break;
            }
            terms.push(Term {
                base: base.unwrap_or(Base::Current),
                offset: offset.unwrap_or(0),
            });
            rest = after;
            if !rest.starts_with(['/', '?']) {
                break;
            }
        }
        let address = (!terms.is_empty()).then_some(Self { terms });
        Ok((address, rest))
    }
}

impl Base {
    fn parse(text: &str) -> Result<(Option<Self>, &str), CommandError> {
        let mut chars = text.chars();
        let base = match chars.next() {
            Some('.') => Self::Current,
            Some('$') => Self::Last,
            Some('\'') => {
                let mark = chars.next().ok_or(CommandError::MarkNotSet)?;
                return Ok((Some(Self::Mark(mark)), chars.as_str()));
            }
            Some(delimiter @ ('/' | '?')) => {
                let (pattern, rest) = split_at_delimiter(chars.as_str(), delimiter);
                let search = Self::Search {
                    pattern,
                    backward: delimiter == '?',
                };
                return Ok((Some(search), rest.unwrap_or_default()));
            }
            Some(c) if c.is_ascii_digit() => {
                let (number, rest) = leading_number(text)?;
                return Ok((number.map(Self::Number), rest));
            }
            _ => return Ok((None, text)),
        };
        Ok((Some(base), chars.as_str()))
    }
}

/// Lo que se suma a una base: `+N`, `-N`, `+` y `-` solos valen 1, y un
/// número pegado a la base es `+N` (`.5`), como en vim.
fn offset(text: &str) -> Result<(Option<isize>, &str), CommandError> {
    let mut offset: Option<isize> = None;
    let mut rest = text;
    loop {
        let after = skip_blanks(rest);
        let (sign, digits) = match after.chars().next() {
            Some(sign @ ('+' | '-')) => (sign, &after[1..]),
            Some(c) if c.is_ascii_digit() => ('+', after),
            _ => return Ok((offset, rest)),
        };
        let (number, after) = leading_number(digits)?;
        let step = isize::try_from(number.unwrap_or(1)).map_err(|_| CommandError::InvalidRange)?;
        let step = if sign == '-' { -step } else { step };
        offset = Some(offset.unwrap_or(0).saturating_add(step));
        rest = after;
    }
}

pub(crate) fn leading_number(text: &str) -> Result<(Option<usize>, &str), CommandError> {
    let digits = text
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(text.len());
    if digits == 0 {
        return Ok((None, text));
    }
    let number = text[..digits]
        .parse()
        .map_err(|_| CommandError::InvalidRange)?;
    Ok((Some(number), &text[digits..]))
}

pub(crate) fn skip_blanks(text: &str) -> &str {
    text.trim_start_matches([' ', '\t'])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn term(base: Base, offset: isize) -> Term {
        Term { base, offset }
    }

    fn address(terms: impl IntoIterator<Item = Term>) -> Address {
        Address {
            terms: terms.into_iter().collect(),
        }
    }

    fn step(address: Option<Address>, moves_cursor: bool) -> Step {
        Step {
            address,
            moves_cursor,
        }
    }

    fn search(pattern: &str, backward: bool) -> Base {
        Base::Search {
            pattern: pattern.to_owned(),
            backward,
        }
    }

    fn parse(text: &str) -> (Option<LineRange>, &str) {
        LineRange::parse(text).unwrap()
    }

    #[test]
    fn reads_every_kind_of_address() {
        let cases = [
            (".", term(Base::Current, 0), ""),
            ("$-1d", term(Base::Last, -1), "d"),
            ("12y", term(Base::Number(12), 0), "y"),
            ("+", term(Base::Current, 1), ""),
            ("-2+1", term(Base::Current, -1), ""),
            (".5", term(Base::Current, 5), ""),
            ("'a+1", term(Base::Mark('a'), 1), ""),
            ("'<", term(Base::Mark('<'), 0), ""),
            ("/a\\/b/-1d", term(search("a\\/b", false), -1), "d"),
            ("?x?s", term(search("x", true), 0), "s"),
            ("/abc", term(search("abc", false), 0), ""),
        ];
        for (text, expected, rest) in cases {
            let range = LineRange::Addresses(vec![step(Some(address([expected])), false)]);
            assert_eq!(parse(text), (Some(range), rest), "{text:?}");
        }
    }

    #[test]
    fn chains_searches_and_separators() {
        let (range, rest) = parse("/a//b/+1;?c?,$d");
        let expected = vec![
            step(
                Some(address([
                    term(search("a", false), 0),
                    term(search("b", false), 1),
                ])),
                true,
            ),
            step(Some(address([term(search("c", true), 0)])), false),
            step(Some(address([term(Base::Last, 0)])), false),
        ];
        assert_eq!((range, rest), (Some(LineRange::Addresses(expected)), "d"));
        let (range, _) = parse(",5");
        let expected = vec![
            step(None, false),
            step(Some(address([term(Base::Number(5), 0)])), false),
        ];
        assert_eq!(range, Some(LineRange::Addresses(expected)));
        assert_eq!(parse("%s"), (Some(LineRange::Whole), "s"));
        assert_eq!(parse("  d"), (None, "d"));
    }

    #[test]
    fn rejects_numbers_that_do_not_fit() {
        assert_eq!(
            LineRange::parse("99999999999999999999999s"),
            Err(CommandError::InvalidRange)
        );
        assert_eq!(
            LineRange::parse(".+99999999999999999999999s"),
            Err(CommandError::InvalidRange)
        );
    }
}
