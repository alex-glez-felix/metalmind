use std::cmp::Ordering;

use metalmind_buffer::{Buffer, ByteOffset};

use super::Ends;
use crate::lines::is_blank;
use crate::span::{Span, SpanKind};

/// `i"`, `a"` con un operador pendiente. Solo mira la línea del cursor, como
/// vim.
pub(super) fn resolve(
    buffer: &Buffer,
    head: ByteOffset,
    count: usize,
    around: bool,
    quote: char,
) -> Option<Span> {
    let line = Line::new(buffer, buffer.line_of(head), quote);
    let cursor = line.index_of(head);
    let selected = Selected::default();
    let (start, end) = delimit(&line, cursor, selected, count, around)?;
    let kind = if around || count > 1 {
        SpanKind::Inclusive
    } else {
        SpanKind::Exclusive
    };
    Some(Span {
        start: line.offset(start),
        end: line.offset(end),
        kind,
    })
}

/// En Visual; solo dentro de una línea. `false` si falló, y entonces no
/// mueve nada.
pub(super) fn select(
    buffer: &Buffer,
    ends: &mut Ends,
    count: usize,
    around: bool,
    quote: char,
) -> bool {
    let line_number = buffer.line_of(ends.head);
    if buffer.line_of(ends.anchor) != line_number {
        return false;
    }
    let line = Line::new(buffer, line_number, quote);
    let (anchor, cursor) = (line.index_of(ends.anchor), line.index_of(ends.head));
    let selected = Selected::of(&line, anchor, cursor);
    let Some((start, end)) = delimit(&line, cursor, selected, count, around) else {
        return false;
    };
    let (anchor, head) = selected.reselect(&line, anchor, (start, end), count, around);
    *ends = Ends {
        anchor: line.offset(anchor),
        head: line.offset(head),
    };
    true
}

/// Dónde está el otro extremo de la selección respecto al cursor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Anchor {
    /// En el cursor: hay un solo carácter seleccionado.
    #[default]
    OnCursor,
    Before,
    After,
}

/// Lo que `current_quote` mira de la selección Visual antes de buscar.
#[derive(Clone, Copy, Debug, Default)]
struct Selected {
    anchor: Anchor,
    /// Abarca justo el interior de unas comillas, como tras `vi"`.
    inside_quotes: bool,
    has_quote: bool,
}

impl Selected {
    fn of(line: &Line, anchor: usize, cursor: usize) -> Self {
        let (side, first, last) = match anchor.cmp(&cursor) {
            Ordering::Equal => return Self::default(),
            Ordering::Less => (Anchor::Before, anchor, cursor),
            Ordering::Greater => (Anchor::After, cursor, anchor),
        };
        Self {
            anchor: side,
            inside_quotes: first > 0 && line.is_quote(first - 1) && line.quote_after(last),
            has_quote: (first..=last)
                .map_while(|index| line.char(index))
                .any(|c| c == line.quote),
        }
    }

    fn is_some(self) -> bool {
        self.anchor != Anchor::OnCursor
    }

    /// Los extremos nuevos, en índices de carácter: tras `vi"`, otro `i"`
    /// toma también las comillas, y una selección que ya tenía comillas
    /// conserva su otro extremo.
    fn reselect(
        self,
        line: &Line,
        anchor: usize,
        (start, end): (usize, usize),
        count: usize,
        around: bool,
    ) -> (usize, usize) {
        let starts_at_quote = line.is_quote(anchor) || (anchor > 0 && line.is_quote(anchor - 1));
        let moves_anchor = match self.anchor {
            Anchor::OnCursor => true,
            Anchor::Before => !self.has_quote && (self.inside_quotes || !starts_at_quote),
            Anchor::After => false,
        };
        let mut new_anchor = if moves_anchor { start } else { anchor };
        let includes_end = around || count > 1 || (self.is_some() && self.inside_quotes);
        let past = if includes_end { end + 1 } else { end };
        if self.anchor != Anchor::After {
            return (new_anchor, past - 1);
        }
        let ends_at_quote = line.is_quote(anchor) || line.quote_after(anchor);
        if self.inside_quotes || (!self.has_quote && !ends_at_quote) {
            new_anchor = past - 1;
        }
        (new_anchor, start)
    }
}

/// Las comillas del objeto y los blancos que `a"` suma, en índices de
/// carácter; sin las comillas en `i"` con count 1.
fn delimit(
    line: &Line,
    cursor: usize,
    selected: Selected,
    count: usize,
    around: bool,
) -> Option<(usize, usize)> {
    let (mut start, mut end) = quotes(line, cursor, selected)?;
    if around {
        if line.char(end + 1).is_some_and(is_blank) {
            while line.char(end + 1).is_some_and(is_blank) {
                end += 1;
            }
        } else {
            while start > 0 && line.char(start - 1).is_some_and(is_blank) {
                start -= 1;
            }
        }
    }
    if !around && count < 2 && !(selected.is_some() && selected.inside_quotes) {
        start += 1;
    }
    Some((start, end))
}

/// La comilla que abre y la que cierra. Si ya hay algo seleccionado y el
/// cursor está sobre una comilla, busca el par siguiente hacia su lado.
fn quotes(line: &Line, cursor: usize, selected: Selected) -> Option<(usize, usize)> {
    let on_quote = line.is_quote(cursor);
    if selected.is_some() && on_quote {
        return if selected.anchor == Anchor::Before {
            let start = line.next_quote(cursor + 1, false)?;
            Some(
                line.next_quote(start + 1, true)
                    .map_or((cursor, start), |end| (start, end)),
            )
        } else {
            let end = line.prev_quote(cursor, false);
            if !line.is_quote(end) {
                return None;
            }
            let start = line.prev_quote(end, true);
            Some(if line.is_quote(start) {
                (start, end)
            } else {
                (end, cursor)
            })
        };
    }
    if on_quote || selected.is_some() {
        let first = match selected.anchor {
            Anchor::OnCursor => cursor,
            Anchor::Before => line.next_quote(cursor, false)?,
            Anchor::After => line.prev_quote(cursor, false),
        };
        return line.pair_containing(first);
    }
    line.pair_around(cursor)
}

/// Caracteres de una línea, indexados por carácter.
struct Line {
    chars: Vec<(ByteOffset, char)>,
    end: ByteOffset,
    quote: char,
}

impl Line {
    fn new(buffer: &Buffer, line: usize, quote: char) -> Self {
        let end = buffer.line_end(line);
        let mut chars = Vec::new();
        let mut offset = buffer.line_start(line);
        while let Some(c) = buffer.char_at(offset).filter(|_| offset < end) {
            chars.push((offset, c));
            offset = offset + c.len_utf8();
        }
        Self { chars, end, quote }
    }

    fn char(&self, index: usize) -> Option<char> {
        self.chars.get(index).map(|&(_, c)| c)
    }

    fn is_quote(&self, index: usize) -> bool {
        self.char(index) == Some(self.quote)
    }

    /// Si el byte que sigue al carácter en `index` es una comilla: vim mira
    /// `line[col + 1]`, que tras un carácter multibyte nunca lo es.
    fn quote_after(&self, index: usize) -> bool {
        self.char(index).is_some_and(|c| c.is_ascii()) && self.is_quote(index + 1)
    }

    fn offset(&self, index: usize) -> ByteOffset {
        self.chars
            .get(index)
            .map_or(self.end, |&(offset, _)| offset)
    }

    fn index_of(&self, offset: ByteOffset) -> usize {
        self.chars.partition_point(|&(at, _)| at < offset)
    }

    /// El cursor está sobre una comilla: se cuentan pares desde el inicio de
    /// la línea para saber si abre o cierra.
    fn pair_containing(&self, cursor: usize) -> Option<(usize, usize)> {
        let mut from = 0;
        loop {
            let start = self.next_quote(from, false)?;
            if start > cursor {
                return None;
            }
            let end = self.next_quote(start + 1, true)?;
            if start <= cursor && cursor <= end {
                return Some((start, end));
            }
            from = end + 1;
        }
    }

    /// La comilla anterior abre; si no hay, se usa la siguiente.
    fn pair_around(&self, cursor: usize) -> Option<(usize, usize)> {
        let mut start = self.prev_quote(cursor, true);
        if self.char(start) != Some(self.quote) {
            start = self.next_quote(start, false)?;
        }
        let end = self.next_quote(start + 1, true)?;
        Some((start, end))
    }

    fn next_quote(&self, from: usize, skip_escaped: bool) -> Option<usize> {
        let mut index = from;
        loop {
            let c = self.char(index)?;
            if skip_escaped && c == '\\' {
                index += 1;
                self.char(index)?;
            } else if c == self.quote {
                return Some(index);
            }
            index += 1;
        }
    }

    /// La comilla anterior, o 0 si no hay.
    fn prev_quote(&self, from: usize, skip_escaped: bool) -> usize {
        let mut index = from;
        while index > 0 {
            index -= 1;
            let mut backslashes = 0;
            while skip_escaped
                && index > backslashes
                && self.char(index - backslashes - 1) == Some('\\')
            {
                backslashes += 1;
            }
            if backslashes % 2 == 1 {
                index -= backslashes;
            } else if self.char(index) == Some(self.quote) {
                break;
            }
        }
        index
    }
}
