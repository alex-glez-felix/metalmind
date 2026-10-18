//! `:sort`: sus opciones y el orden, como `ex_sort` de vim.

mod number;

use std::cmp::Ordering;
use std::ops::Range;

use self::number::Radix;
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
    /// `n`, `b`, `o` o `x`: el primer número en esa base; las líneas sin
    /// número van antes.
    Integer(Radix),
    /// `f`: el número con que empieza, como `strtod`.
    Float,
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
    /// Las opciones van en cualquier orden, con o sin espacios, y un `"`
    /// empieza un comentario, como en `ex_sort` de vim.
    ///
    /// # Errors
    /// Con una opción que vim no tiene, o que Metalmind no soporta todavía.
    pub fn parse(reverse: bool, args: &str) -> Result<Self, CommandError> {
        let mut options = Self {
            reverse,
            ..Self::default()
        };
        let mut by_match = false;
        let mut source = None;
        let mut formats = 0;
        let mut rest = args;
        while let Some(c) = rest.chars().next() {
            let after = &rest[c.len_utf8()..];
            rest = match c {
                ' ' | '\t' => after,
                'i' => set(&mut options.ignore_case, after),
                'u' => set(&mut options.unique, after),
                'r' => set(&mut by_match, after),
                'n' | 'b' | 'o' | 'x' | 'f' => {
                    options.by = SortBy::from_flag(c);
                    formats += 1;
                    after
                }
                'l' => return Err(CommandError::UnsupportedSortFlag(c)),
                '"' => break,
                delimiter if !delimiter.is_ascii_alphabetic() && source.is_none() => {
                    let (pattern, rest) = split_at_delimiter(after, delimiter);
                    source = Some(pattern);
                    rest.ok_or_else(|| CommandError::MissingDelimiter(after.to_owned()))?
                }
                _ => return Err(CommandError::InvalidArgumentAt(rest.to_owned())),
            };
        }
        if formats > 1 {
            return Err(CommandError::InvalidArgument);
        }
        options.pattern = source.map(|source| SortPattern { source, by_match });
        Ok(options)
    }
}

impl SortBy {
    fn from_flag(flag: char) -> Self {
        match flag {
            'b' => Self::Integer(Radix::Binary),
            'o' => Self::Integer(Radix::Octal),
            'x' => Self::Integer(Radix::Hex),
            'f' => Self::Float,
            _ => Self::Integer(Radix::Decimal),
        }
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
    key: impl Fn(&str) -> Range<usize>,
) -> Vec<&'a str> {
    let keys = lines.iter().map(|line| &line[key(line)]);
    let mut order: Vec<usize> = (0..lines.len()).collect();
    // Estable: las líneas iguales conservan su orden, como en vim.
    match options.by {
        SortBy::Text => {
            let keys: Vec<&str> = keys.collect();
            order.sort_by(|&a, &b| compare(keys[a], keys[b], options.ignore_case));
        }
        SortBy::Integer(radix) => {
            let keys: Vec<Option<i64>> =
                keys.map(|key| number::first_integer(key, radix)).collect();
            order.sort_by_key(|&index| keys[index]);
        }
        SortBy::Float => {
            let keys: Vec<f64> = keys.map(number::float).collect();
            order.sort_by(|&a, &b| number::compare_floats(keys[a], keys[b]));
        }
    }
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
        assert_eq!(options("n").by, SortBy::Integer(Radix::Decimal));
        assert_eq!(options("x").by, SortBy::Integer(Radix::Hex));
        assert_eq!(options("f").by, SortBy::Float);
        assert_eq!(options("n").pattern, None);
        assert_eq!(options("u \" n").by, SortBy::Text);
        let errors = [
            ("l", CommandError::UnsupportedSortFlag('l')),
            ("zq", CommandError::InvalidArgumentAt("zq".to_owned())),
            ("/a/ /b/", CommandError::InvalidArgumentAt("/b/".to_owned())),
            ("n x", CommandError::InvalidArgument),
            ("/a", CommandError::MissingDelimiter("a".to_owned())),
        ];
        for (args, error) in errors {
            assert_eq!(SortOptions::parse(false, args), Err(error), "{args:?}");
        }
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
    fn sorts_by_numbers_in_any_base() {
        let lines = ["0x1F", "a", "-3", "0b10", "10"];
        assert_eq!(sorted(&lines, "x"), ["-3", "a", "10", "0x1F", "0b10"]);
        assert_eq!(sorted(&lines, "b"), ["a", "-3", "0x1F", "0b10", "10"]);
        assert_eq!(
            sorted(&["2.5", "", "-1e1", "x"], "f"),
            ["", "-1e1", "x", "2.5"]
        );
        let floats = ["inf", "0x0p1024", "1"];
        assert_eq!(sorted(&floats, "f"), ["0x0p1024", "1", "inf"]);
    }
}
