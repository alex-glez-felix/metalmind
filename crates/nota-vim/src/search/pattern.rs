use std::ops::Range;

use nota_buffer::{Buffer, ByteOffset};
use regex::{Regex, RegexBuilder};

use super::SearchError;
use super::translate::{has_uppercase, translate};

/// Cómo trata las mayúsculas un patrón sin `\c` ni `\C`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Case {
    /// `ignorecase` + `smartcase`: distingue solo si el patrón tiene mayúsculas.
    Smart,
    /// `ignorecase` sin `smartcase`, como `*` y `#`.
    Ignore,
}

/// Un patrón de búsqueda ya compilado, con el texto tal como se escribió.
/// Busca línea por línea, como vim: `^` y `$` son inicio y fin de línea.
#[derive(Clone, Debug)]
pub struct Pattern {
    source: String,
    regex: Regex,
}

impl Pattern {
    pub(crate) fn compile(source: &str, case: Case) -> Result<Self, SearchError> {
        let translation = translate(source)?;
        let ignore_case = translation.ignore_case.unwrap_or(match case {
            Case::Smart => !has_uppercase(source),
            Case::Ignore => true,
        });
        let regex = RegexBuilder::new(&translation.regex)
            .case_insensitive(ignore_case)
            .build()
            .map_err(|_| SearchError::Invalid(source.to_owned()))?;
        Ok(Self {
            source: source.to_owned(),
            regex,
        })
    }

    /// Lo que se escribió, en el dialecto de vim.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Las coincidencias en las líneas `lines`, para resaltarlas.
    #[must_use]
    pub fn matches_in(&self, buffer: &Buffer, lines: Range<usize>) -> Vec<Range<ByteOffset>> {
        let mut found = Vec::new();
        for line in lines.start..lines.end.min(buffer.line_count()) {
            let start = buffer.line_start(line);
            let text = buffer.line_text(line);
            found.extend(
                self.line_matches(&text)
                    .map(|range| start + range.start..start + range.end),
            );
        }
        found
    }

    /// Las coincidencias de una línea en el orden en que las recorre la
    /// búsqueda de vim: sin solaparse, y tras una vacía sigue un carácter
    /// después (`cpoptions` con `c`).
    pub(crate) fn line_matches<'a>(&'a self, line: &'a str) -> impl Iterator<Item = Range<usize>> {
        let mut next = Some(0);
        std::iter::from_fn(move || {
            let found = self.regex.find_at(line, next?)?.range();
            next = resume_after(line, &found);
            Some(found)
        })
    }
}

/// Dónde sigue buscando vim tras `found`; `None` si ya llegó al fin de línea.
fn resume_after(line: &str, found: &Range<usize>) -> Option<usize> {
    let next = if found.is_empty() {
        line[found.end..]
            .chars()
            .next()
            .map_or(line.len(), |c| found.end + c.len_utf8())
    } else {
        found.end
    };
    (next < line.len()).then_some(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pattern(source: &str) -> Pattern {
        Pattern::compile(source, Case::Smart).unwrap()
    }

    fn spans(ranges: impl Iterator<Item = Range<usize>>) -> Vec<(usize, usize)> {
        ranges.map(|range| (range.start, range.end)).collect()
    }

    #[test]
    fn smartcase_only_matches_case_with_an_uppercase() {
        let lower = pattern("foo");
        assert_eq!(lower.line_matches("Foo foo").count(), 2);
        let upper = pattern("Foo");
        assert_eq!(spans(upper.line_matches("Foo foo")), [(0, 3)]);
        let forced = pattern("\\cFoo");
        assert_eq!(forced.line_matches("Foo foo").count(), 2);
        let ignore = Pattern::compile("Foo", Case::Ignore).unwrap();
        assert_eq!(ignore.line_matches("Foo foo").count(), 2);
    }

    #[test]
    fn walks_a_line_like_vim() {
        let empty = pattern("x*");
        assert_eq!(spans(empty.line_matches("ab")), [(0, 0), (1, 1)]);
        let runs = pattern("a*");
        assert_eq!(spans(runs.line_matches("baab")), [(0, 0), (1, 3), (3, 3)]);
        let multibyte = pattern("x*");
        assert_eq!(spans(multibyte.line_matches("ñé")), [(0, 0), (2, 2)]);
        let end = pattern("$");
        assert_eq!(spans(end.line_matches("abc")), [(3, 3)]);
        assert_eq!(spans(end.line_matches("")), [(0, 0)]);
    }

    #[test]
    fn finds_matches_in_visible_lines() {
        let buffer = Buffer::from("ab\nxab ab\nab");
        let found = pattern("ab").matches_in(&buffer, 1..9);
        let offsets: Vec<_> = found.iter().map(|r| (r.start.get(), r.end.get())).collect();
        assert_eq!(offsets, [(4, 6), (7, 9), (10, 12)]);
    }
}
