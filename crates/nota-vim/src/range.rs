//! Rangos de líneas de `:` (`%`, `3`, `.,$`, `.-1,.+2`).

use std::ops::RangeInclusive;

use crate::CommandError;

/// Un rango tal como se escribió, antes de saber en qué línea está el cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LineRange {
    Whole,
    Between(Address, Address),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Address {
    base: Base,
    offset: isize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Base {
    Current,
    Last,
    /// Desde 1, como se escribe.
    Number(usize),
}

impl LineRange {
    /// El rango al inicio de `text`, si hay, y lo que sigue.
    ///
    /// # Errors
    /// [`CommandError::InvalidRange`] si un número no cabe.
    pub fn parse(text: &str) -> Result<(Option<Self>, &str), CommandError> {
        if let Some(rest) = text.strip_prefix('%') {
            return Ok((Some(Self::Whole), rest));
        }
        let (first, rest) = Address::parse(text)?;
        let Some(rest) = rest.strip_prefix(',') else {
            return Ok((first.map(|first| Self::Between(first, first)), rest));
        };
        let current = Address {
            base: Base::Current,
            offset: 0,
        };
        let (second, rest) = Address::parse(rest)?;
        let range = Self::Between(first.unwrap_or(current), second.unwrap_or(current));
        Ok((Some(range), rest))
    }

    /// Las líneas, desde 0, con el cursor en `current`.
    ///
    /// # Errors
    /// Si se sale del texto (E16) o va hacia atrás (E493).
    pub fn resolve(
        self,
        current: usize,
        last: usize,
    ) -> Result<RangeInclusive<usize>, CommandError> {
        let (first, second) = match self {
            Self::Whole => return Ok(0..=last),
            Self::Between(first, second) => (
                first.resolve(current, last)?,
                second.resolve(current, last)?,
            ),
        };
        if first > second {
            return Err(CommandError::BackwardsRange);
        }
        Ok(first..=second)
    }
}

impl Address {
    fn parse(text: &str) -> Result<(Option<Self>, &str), CommandError> {
        let (base, mut rest) = if let Some(rest) = text.strip_prefix('.') {
            (Some(Base::Current), rest)
        } else if let Some(rest) = text.strip_prefix('$') {
            (Some(Base::Last), rest)
        } else {
            let (number, rest) = leading_number(text)?;
            (number.map(Base::Number), rest)
        };
        let mut offset: Option<isize> = None;
        while let Some(sign) = rest.chars().next().filter(|c| matches!(c, '+' | '-')) {
            let (number, after) = leading_number(&rest[1..])?;
            let step =
                isize::try_from(number.unwrap_or(1)).map_err(|_| CommandError::InvalidRange)?;
            let step = if sign == '-' { -step } else { step };
            offset = Some(offset.unwrap_or(0).saturating_add(step));
            rest = after;
        }
        let address = match (base, offset) {
            (None, None) => None,
            (base, offset) => Some(Self {
                base: base.unwrap_or(Base::Current),
                offset: offset.unwrap_or(0),
            }),
        };
        Ok((address, rest))
    }

    fn resolve(self, current: usize, last: usize) -> Result<usize, CommandError> {
        let base = match self.base {
            Base::Current => current,
            Base::Last => last,
            // Como en vim, la línea 0 vale como la 1.
            Base::Number(number) => number.saturating_sub(1),
        };
        base.checked_add_signed(self.offset)
            .filter(|&line| line <= last)
            .ok_or(CommandError::InvalidRange)
    }
}

fn leading_number(text: &str) -> Result<(Option<usize>, &str), CommandError> {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Con el cursor en la línea 2 (desde 0) de un texto de 5 líneas.
    fn lines(text: &str) -> Result<(Option<RangeInclusive<usize>>, &str), CommandError> {
        let (range, rest) = LineRange::parse(text)?;
        let lines = range.map(|range| range.resolve(2, 4)).transpose()?;
        Ok((lines, rest))
    }

    #[test]
    fn parses_like_vim() {
        let cases = [
            ("s", None, "s"),
            ("%s", Some(0..=4), "s"),
            ("2s", Some(1..=1), "s"),
            ("2,4s", Some(1..=3), "s"),
            (".,$s", Some(2..=4), "s"),
            (".-1,.+1s", Some(1..=3), "s"),
            ("-,+s", Some(1..=3), "s"),
            ("+2s", Some(4..=4), "s"),
            ("$-1s", Some(3..=3), "s"),
            ("0s", Some(0..=0), "s"),
            ("3,s", Some(2..=2), "s"),
            (",4s", Some(2..=3), "s"),
        ];
        for (text, expected, rest) in cases {
            assert_eq!(lines(text), Ok((expected, rest)), "{text:?}");
        }
    }

    #[test]
    fn rejects_lines_outside_the_text() {
        assert_eq!(lines("6s"), Err(CommandError::InvalidRange));
        assert_eq!(lines(".+3s"), Err(CommandError::InvalidRange));
        assert_eq!(lines("1-2s"), Err(CommandError::InvalidRange));
        assert_eq!(lines("4,2s"), Err(CommandError::BackwardsRange));
        assert_eq!(
            lines("99999999999999999999999s"),
            Err(CommandError::InvalidRange)
        );
    }
}
