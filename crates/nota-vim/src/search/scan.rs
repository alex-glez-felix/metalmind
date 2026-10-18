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
        let below = (line + 1..buffer.line_count()).chain(0..=line);
        below.enumerate().find_map(|(walked, other)| {
            let found = self.line_matches(&buffer.line_text(other)).next()?;
            let wrapped = (walked + line + 1 >= buffer.line_count()).then_some(Wrap::ToTop);
            Some(hit(buffer.line_start(other) + found.start, wrapped))
        })
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
        let above = (0..line).rev().chain((line..buffer.line_count()).rev());
        above.enumerate().find_map(|(walked, other)| {
            let found = self.line_matches(&buffer.line_text(other)).last()?;
            let wrapped = (walked >= line).then_some(Wrap::ToBottom);
            Some(hit(buffer.line_start(other) + found.start, wrapped))
        })
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
}
