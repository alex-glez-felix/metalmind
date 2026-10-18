//! Recorrer el buffer desde el cursor con la lógica de `searchit()` de vim,
//! dando la vuelta al llegar a un borde (`wrapscan`).

use nota_buffer::{Buffer, ByteOffset};

use super::Pattern;
use crate::motion::Destination;
use crate::span::SpanKind;

/// Una búsqueda llegó a un borde del texto y siguió por el otro.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wrap {
    /// "search hit BOTTOM, continuing at TOP"
    ToTop,
    /// "search hit TOP, continuing at BOTTOM"
    ToBottom,
}

/// Dónde empieza la coincidencia encontrada.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Hit {
    pub offset: ByteOffset,
    pub wrapped: Option<Wrap>,
}

impl Hit {
    /// Como motion de un operador, la búsqueda es exclusiva.
    pub fn destination(self) -> Destination {
        Destination::new(self.offset, SpanKind::Exclusive)
    }
}

/// Un patrón con su dirección: lo que buscan `/`, `?`, `n` y `N`.
#[derive(Clone, Debug)]
pub(crate) struct Query {
    pub pattern: Pattern,
    pub backward: bool,
}

impl Query {
    /// La `times`-ésima coincidencia desde `from`; `None` si no hay ninguna.
    pub fn find(&self, buffer: &Buffer, from: ByteOffset, times: usize) -> Option<Hit> {
        let mut hit = Hit {
            offset: from,
            wrapped: None,
        };
        for _ in 0..times.max(1) {
            let next = if self.backward {
                self.pattern.backward_from(buffer, hit.offset)
            } else {
                self.pattern.forward_from(buffer, hit.offset)
            }?;
            hit = Hit {
                offset: next.offset,
                wrapped: hit.wrapped.or(next.wrapped),
            };
        }
        Some(hit)
    }
}

impl Pattern {
    /// La primera coincidencia después del carácter bajo el cursor.
    fn forward_from(&self, buffer: &Buffer, from: ByteOffset) -> Option<Hit> {
        let line = buffer.line_of(from);
        let start = buffer.line_start(line);
        let column = from.get() - start.get();
        let text = buffer.line_text(line);
        let under_cursor = text[column..].chars().next().map_or(1, char::len_utf8);
        // Como vim, una coincidencia en el fin de línea cuenta como si
        // estuviera sobre el último carácter.
        let after_cursor = self.line_matches(&text).find(|found| {
            found.start >= column + under_cursor + usize::from(found.start == text.len())
        });
        if let Some(found) = after_cursor {
            return Some(hit(start + found.start, None));
        }
        let first_in = |lines| self.matching_lines(buffer, lines).next();
        let (other, wrapped) = first_in(line + 1..buffer.line_count())
            .map(|other| (other, None))
            .or_else(|| Some((first_in(0..line + 1)?, Some(Wrap::ToTop))))?;
        let found = self.line_matches(&buffer.line_text(other)).next()?;
        Some(hit(buffer.line_start(other) + found.start, wrapped))
    }

    /// La última coincidencia antes del cursor. Desde la columna 0, vim
    /// empieza por la línea de arriba.
    fn backward_from(&self, buffer: &Buffer, from: ByteOffset) -> Option<Hit> {
        let line = buffer.line_of(from);
        let start = buffer.line_start(line);
        let column = from.get() - start.get();
        if column > 0 {
            let text = buffer.line_text(line);
            let before_cursor = self
                .line_matches(&text)
                .filter(|found| found.start < column + usize::from(found.start == text.len()))
                .last();
            if let Some(found) = before_cursor {
                return Some(hit(start + found.start, None));
            }
        }
        let (other, wrapped) = self
            .last_matching_line(buffer, 0..line)
            .map(|other| (other, None))
            .or_else(|| {
                let other = self.last_matching_line(buffer, line..buffer.line_count())?;
                Some((other, Some(Wrap::ToBottom)))
            })?;
        let found = self.line_matches(&buffer.line_text(other)).last()?;
        Some(hit(buffer.line_start(other) + found.start, wrapped))
    }
}

fn hit(offset: ByteOffset, wrapped: Option<Wrap>) -> Hit {
    Hit { offset, wrapped }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::Case;

    fn find(text: &str, from: usize, source: &str, backward: bool) -> Option<(usize, bool)> {
        let query = Query {
            pattern: Pattern::compile(source, Case::Smart).unwrap(),
            backward,
        };
        let buffer = Buffer::from(text);
        query
            .find(&buffer, ByteOffset::new(from), 1)
            .map(|hit| (hit.offset.get(), hit.wrapped.is_some()))
    }

    #[test]
    fn wraps_only_after_passing_an_edge() {
        assert_eq!(find("ab\nab", 0, "b", false), Some((1, false)));
        assert_eq!(find("ab\nab", 1, "b", false), Some((4, false)));
        assert_eq!(find("ab\nab", 4, "b", false), Some((1, true)));
        assert_eq!(find("ab\nab", 4, "b", true), Some((1, false)));
        assert_eq!(find("ab\nab", 1, "b", true), Some((4, true)));
        assert_eq!(find("ab", 1, "b", false), Some((1, true)));
        assert_eq!(find("ab", 0, "z", false), None);
    }

    /// La búsqueda de antes, línea por línea, como referencia.
    fn line_by_line(
        buffer: &Buffer,
        pattern: &Pattern,
        from: usize,
        backward: bool,
    ) -> Option<usize> {
        let line = buffer.line_of(ByteOffset::new(from));
        let start = buffer.line_start(line).get();
        let column = from - start;
        let text = buffer.line_text(line);
        let mut in_line = pattern.line_matches(&text).map(|found| found.start);
        let in_line = if backward {
            in_line
                .filter(|&at| column > 0 && at < column + usize::from(at == text.len()))
                .last()
        } else {
            let under_cursor = text[column..].chars().next().map_or(1, char::len_utf8);
            in_line.find(|&at| at >= column + under_cursor + usize::from(at == text.len()))
        };
        if let Some(at) = in_line {
            return Some(start + at);
        }
        let count = buffer.line_count();
        let others: Vec<usize> = if backward {
            (0..line).rev().chain((line..count).rev()).collect()
        } else {
            (line + 1..count).chain(0..=line).collect()
        };
        others.into_iter().find_map(|other| {
            let text = buffer.line_text(other);
            let mut found = pattern.line_matches(&text);
            let found = if backward { found.last() } else { found.next() }?;
            Some(buffer.line_start(other).get() + found.start)
        })
    }

    /// En una nota de varios bloques, buscar por bloques encuentra lo mismo
    /// que buscar en cada línea, de ida y de vuelta.
    #[test]
    fn blocks_find_the_same_as_line_by_line() {
        let text: String = (0..12_000)
            .map(|line| match line % 7 {
                0 => String::new(),
                3 => format!("- [ ] tarea {line} ñandú\t"),
                5 if line % 2_000 == 5 => format!("ver [[otra]]z {line}"),
                _ => format!("nota {line} con [[enlace-{}]]", line % 97),
            })
            .collect::<Vec<_>>()
            .join("\n");
        let buffer = Buffer::from(text.as_str());
        let sources = [
            "^$",
            "$",
            "ú[^a]",
            "\\]\\][^x]",
            "\\<tarea\\>",
            "1\\d\\{3}$",
            "^-",
            "x*",
            "11999",
            "zzz",
        ];
        for source in sources {
            let pattern = Pattern::compile(source, Case::Smart).unwrap();
            for line in (0..buffer.line_count()).step_by(997) {
                let start = buffer.line_start(line);
                let middle = start + buffer.line_text(line).len().min(5);
                for from in [start, middle] {
                    for backward in [false, true] {
                        let query = Query {
                            pattern: pattern.clone(),
                            backward,
                        };
                        let found = query.find(&buffer, from, 1).map(|hit| hit.offset.get());
                        let expected = line_by_line(&buffer, &pattern, from.get(), backward);
                        assert_eq!(found, expected, "{source:?} desde {from:?}, ?: {backward}");
                    }
                }
            }
        }
    }
}
