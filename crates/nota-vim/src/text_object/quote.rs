use nota_buffer::{Buffer, ByteOffset};

use crate::lines::is_blank;
use crate::span::{Span, SpanKind};

/// `i"`, `a"`. Solo mira la línea del cursor, como vim.
pub(super) fn select(
    buffer: &Buffer,
    head: ByteOffset,
    count: usize,
    around: bool,
    quote: char,
) -> Option<Span> {
    let line = Line::new(buffer, buffer.line_of(head), quote);
    let cursor = line.index_of(head);
    let (mut start, mut end) = if line.char(cursor) == Some(quote) {
        line.pair_containing(cursor)?
    } else {
        line.pair_around(cursor)?
    };
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
    let include_quotes = around || count > 1;
    if !include_quotes {
        start += 1;
    }
    let kind = if include_quotes {
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
        let mut start = self.prev_quote(cursor);
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

    fn prev_quote(&self, from: usize) -> usize {
        let mut index = from;
        while index > 0 {
            index -= 1;
            let mut backslashes = 0;
            while index > backslashes && self.char(index - backslashes - 1) == Some('\\') {
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
