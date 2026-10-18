use std::ops::Range;

use nota_buffer::{Buffer, ByteOffset};
use regex::{Captures, Regex, RegexBuilder};

use super::SearchError;
use super::blocks::{blocks, blocks_rev};
use super::translate::{has_uppercase, translate};

/// Cómo trata las mayúsculas un patrón sin `\c` ni `\C`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Case {
    /// `ignorecase` + `smartcase`: distingue solo si el patrón tiene mayúsculas.
    Smart,
    /// `ignorecase` sin `smartcase`, como `*` y `#` o el flag `i` de `:s`.
    Ignore,
    /// El flag `I` de `:s`.
    Match,
}

/// Un patrón de búsqueda ya compilado, con el texto tal como se escribió.
/// Como en vim, `^` y `$` son inicio y fin de línea y ninguna coincidencia
/// cruza un `\n`: buscar en varias líneas seguidas encuentra lo mismo que
/// buscar en cada una.
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
            Case::Match => false,
        });
        let regex = RegexBuilder::new(&translation.regex)
            .case_insensitive(ignore_case)
            .multi_line(true)
            .build()
            .map_err(|_| SearchError::Invalid(source.to_owned()))?;
        Ok(Self {
            source: source.to_owned(),
            regex,
        })
    }

    /// El mismo patrón con otra regla de mayúsculas.
    pub(crate) fn with_case(&self, case: Case) -> Result<Self, SearchError> {
        Self::compile(&self.source, case)
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

    /// Las líneas de `lines` con alguna coincidencia, de arriba abajo.
    pub(crate) fn matching_lines<'a>(
        &'a self,
        buffer: &'a Buffer,
        lines: Range<usize>,
    ) -> impl Iterator<Item = usize> + 'a {
        blocks(buffer, lines).flat_map(move |block| self.matching_lines_in(buffer, block))
    }

    /// La última línea de `lines` con alguna coincidencia.
    pub(crate) fn last_matching_line(&self, buffer: &Buffer, lines: Range<usize>) -> Option<usize> {
        blocks_rev(buffer, lines).find_map(|block| self.matching_lines_in(buffer, block).last())
    }

    /// Un solo regex por bloque; tras cada coincidencia salta a la línea
    /// siguiente, porque basta con saber que la línea tiene una. Las líneas
    /// se cuentan en el texto del bloque, sin volver a preguntarle al rope.
    fn matching_lines_in<'a>(
        &'a self,
        buffer: &'a Buffer,
        block: Range<usize>,
    ) -> impl Iterator<Item = usize> + 'a {
        let mut line = block.start;
        let text = buffer.lines_text(block);
        let mut next = Some(0);
        std::iter::from_fn(move || {
            let from = next?;
            let found = self.regex.find_at(&text, from)?;
            let skipped = text.as_bytes()[from..found.start()]
                .iter()
                .filter(|&&b| b == b'\n');
            let matched = line + skipped.count();
            next = text[found.end()..]
                .find('\n')
                .map(|at| found.end() + at + 1);
            line = matched + 1;
            Some(matched)
        })
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

    /// Lo que reemplaza `:s` en una línea: como en la búsqueda, pero una
    /// coincidencia vacía justo donde terminó otra no cuenta (`do_sub()`).
    pub(crate) fn line_captures<'a>(&'a self, line: &'a str) -> impl Iterator<Item = Captures<'a>> {
        let mut next = Some(0);
        let mut previous_end = None;
        std::iter::from_fn(move || {
            loop {
                let captures = self.regex.captures_at(line, next?)?;
                let found = captures.get(0)?.range();
                let follows_previous = found.is_empty() && previous_end == Some(found.start);
                next = resume_after(line, &found);
                previous_end = Some(found.end);
                if !follows_previous {
                    return Some(captures);
                }
            }
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
        let exact = Pattern::compile("foo", Case::Match).unwrap();
        assert_eq!(spans(exact.line_matches("Foo foo")), [(4, 7)]);
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
    fn substitution_skips_an_empty_match_right_after_another() {
        let ranges = |source, line| {
            let pattern = pattern(source);
            pattern
                .line_captures(line)
                .map(|c| {
                    let found = c.get(0).unwrap();
                    (found.start(), found.end())
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(ranges("a*", "ab"), [(0, 1)]);
        assert_eq!(ranges("a*", "baab"), [(0, 0), (1, 3)]);
        assert_eq!(ranges("a\\=", "baab"), [(0, 0), (1, 2), (2, 3)]);
    }

    #[test]
    fn finds_matches_in_visible_lines() {
        let buffer = Buffer::from("ab\nxab ab\nab");
        let found = pattern("ab").matches_in(&buffer, 1..9);
        let offsets: Vec<_> = found.iter().map(|r| (r.start.get(), r.end.get())).collect();
        assert_eq!(offsets, [(4, 6), (7, 9), (10, 12)]);
    }
}
