//! `:sort`: sus opciones y el orden, como `ex_sort` de vim.

use std::cmp::Ordering;

use crate::CommandError;
use crate::search::split_at_delimiter;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SortOptions {
    /// `!`
    pub reverse: bool,
    /// `i`
    pub ignore_case: bool,
    /// `u`: de las líneas iguales queda la primera.
    pub unique: bool,
    pub by: SortBy,
    pub pattern: Option<SortPattern>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SortBy {
    #[default]
    Text,
    /// `n`: el primer número decimal; las líneas sin número van antes.
    Number,
}

/// `/patrón/`: se ordena por lo que sigue a su primera coincidencia o, con
/// `r`, por la coincidencia. Las líneas donde no está van antes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SortPattern {
    /// Vacío es el último patrón.
    pub source: String,
    pub by_match: bool,
}

impl SortOptions {
    /// Las opciones van en cualquier orden, con o sin espacios.
    ///
    /// # Errors
    /// Con una opción que vim no tiene, o que nota no soporta todavía.
    pub fn parse(reverse: bool, args: &str) -> Result<Self, CommandError> {
        let mut options = Self {
            reverse,
            ..Self::default()
        };
        let mut by_match = false;
        let mut source = None;
        let mut rest = args;
        while let Some(c) = rest.chars().next() {
            let after = &rest[c.len_utf8()..];
            rest = match c {
                ' ' | '\t' => after,
                'i' => set(&mut options.ignore_case, after),
                'u' => set(&mut options.unique, after),
                'r' => set(&mut by_match, after),
                'n' => {
                    options.by = SortBy::Number;
                    after
                }
                'b' | 'f' | 'o' | 'x' => return Err(CommandError::UnsupportedSortFlag(c)),
                c if c.is_ascii_alphabetic() || c == '"' || c == '\\' => {
                    return Err(CommandError::InvalidArgument);
                }
                delimiter => {
                    let (pattern, after) = split_at_delimiter(after, delimiter);
                    source = Some(pattern);
                    after.ok_or_else(|| CommandError::MissingDelimiter(rest.to_owned()))?
                }
            };
        }
        options.pattern = source.map(|source| SortPattern { source, by_match });
        Ok(options)
    }
}

fn set<'a>(flag: &mut bool, rest: &'a str) -> &'a str {
    *flag = true;
    rest
}

/// Las líneas ordenadas. `key` da el trozo de cada línea por el que se
/// ordena: lo que sigue al patrón o lo que coincide, o vacío si no está.
pub(crate) fn sort<'a>(
    lines: &[&'a str],
    options: &SortOptions,
    key: impl Fn(&str) -> std::ops::Range<usize>,
) -> Vec<&'a str> {
    let keys: Vec<Key> = lines
        .iter()
        .map(|line| Key::of(&line[key(line)], options))
        .collect();
    let mut order: Vec<usize> = (0..lines.len()).collect();
    // Estable: las líneas iguales conservan su orden, como en vim.
    order.sort_by(|&a, &b| keys[a].cmp(&keys[b], options.ignore_case));
    if options.reverse {
        order.reverse();
    }
    let mut sorted: Vec<&str> = Vec::with_capacity(order.len());
    for index in order {
        let line = lines[index];
        let repeated = sorted
            .last()
            .is_some_and(|last| compare(last, line, options.ignore_case).is_eq());
        if !(options.unique && repeated) {
            sorted.push(line);
        }
    }
    sorted
}

enum Key<'a> {
    Text(&'a str),
    /// Sin número va antes que cualquier número.
    Number(Option<i64>),
}

impl<'a> Key<'a> {
    fn of(text: &'a str, options: &SortOptions) -> Self {
        match options.by {
            SortBy::Number => Self::Number(first_number(text)),
            SortBy::Text => Self::Text(text),
        }
    }

    fn cmp(&self, other: &Self, ignore_case: bool) -> Ordering {
        match (self, other) {
            (Self::Number(a), Self::Number(b)) => a.cmp(b),
            (Self::Text(a), Self::Text(b)) => compare(a, b, ignore_case),
            (Self::Number(_), Self::Text(_)) => Ordering::Less,
            (Self::Text(_), Self::Number(_)) => Ordering::Greater,
        }
    }
}

/// Por bytes, como `strcmp`; con `i`, ignorando las mayúsculas ASCII, como
/// `STRICMP` de vim.
fn compare(a: &str, b: &str, ignore_case: bool) -> Ordering {
    if ignore_case {
        let a = a.bytes().map(|b| b.to_ascii_lowercase());
        a.cmp(b.bytes().map(|b| b.to_ascii_lowercase()))
    } else {
        a.cmp(b)
    }
}

/// El primer número decimal, con su `-` si lo lleva justo antes. Como
/// `strtoll`, uno que no cabe se queda en el máximo.
fn first_number(text: &str) -> Option<i64> {
    let start = text.find(|c: char| c.is_ascii_digit())?;
    let digits = &text[start..];
    let len = digits
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(digits.len());
    let negative = text[..start].ends_with('-');
    let magnitude = digits[..len].parse::<i64>().unwrap_or(i64::MAX);
    Some(if negative { -magnitude } else { magnitude })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(args: &str) -> SortOptions {
        SortOptions::parse(false, args).unwrap()
    }

    fn sorted<'a>(lines: &[&'a str], args: &str) -> Vec<&'a str> {
        sort(lines, &options(args), |line| 0..line.len())
    }

    #[test]
    fn parses_flags_in_any_order_and_a_pattern() {
        let parsed = options(" u i/a b/ r");
        assert!(parsed.unique && parsed.ignore_case);
        let pattern = SortPattern {
            source: "a b".to_owned(),
            by_match: true,
        };
        assert_eq!(parsed.pattern, Some(pattern));
        assert_eq!(options("n").by, SortBy::Number);
        assert_eq!(options("n").pattern, None);
        assert_eq!(
            SortOptions::parse(false, "x"),
            Err(CommandError::UnsupportedSortFlag('x'))
        );
        assert_eq!(
            SortOptions::parse(false, "z"),
            Err(CommandError::InvalidArgument)
        );
        assert_eq!(
            SortOptions::parse(false, "/a"),
            Err(CommandError::MissingDelimiter("/a".to_owned()))
        );
    }

    #[test]
    fn sorts_like_vim() {
        let lines = ["c", "B", "a", "b", "A"];
        assert_eq!(sorted(&lines, ""), ["A", "B", "a", "b", "c"]);
        assert_eq!(sorted(&lines, "i"), ["a", "A", "B", "b", "c"]);
        assert_eq!(sorted(&["c", "b", "a", "b", "B"], "ui"), ["a", "b", "c"]);
        let numbers = ["x10", "x9", "y", "x-3", "z", "2"];
        assert_eq!(sorted(&numbers, "n"), ["y", "z", "x-3", "2", "x9", "x10"]);
    }

    #[test]
    fn reversing_also_reverses_equal_lines() {
        let numbers = ["x10", "x9", "y", "x-3", "z", "2"];
        let reversed = sort(&numbers, &SortOptions::parse(true, "n").unwrap(), |line| {
            0..line.len()
        });
        assert_eq!(reversed, ["x10", "x9", "2", "x-3", "z", "y"]);
    }

    #[test]
    fn a_number_that_does_not_fit_is_the_largest() {
        assert_eq!(first_number("a99999999999999999999"), Some(i64::MAX));
        assert_eq!(first_number("a-7b"), Some(-7));
        assert_eq!(first_number("ab"), None);
    }
}
