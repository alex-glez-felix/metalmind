use std::ops::Range;

use nota_buffer::{Buffer, ByteOffset};

use crate::lines::in_indent;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SpanKind {
    Exclusive,
    Inclusive,
    Linewise,
}

/// Lo que recorrió una motion o abarca un text object, en términos de vim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Span {
    pub start: ByteOffset,
    pub end: ByteOffset,
    pub kind: SpanKind,
}

impl Span {
    pub fn between(a: ByteOffset, b: ByteOffset, kind: SpanKind) -> Self {
        Self {
            start: a.min(b),
            end: a.max(b),
            kind,
        }
    }

    pub fn into_target(self, buffer: &Buffer) -> Target {
        let lines = |first, last| Target {
            region: Region::Lines { first, last },
            start: self.start,
            adjusted: false,
        };
        let chars = |end, adjusted| Target {
            region: Region::Chars(self.start..end),
            start: self.start,
            adjusted,
        };
        let first = buffer.line_of(self.start);
        let last = buffer.line_of(self.end);
        match self.kind {
            SpanKind::Linewise => lines(first, last),
            SpanKind::Inclusive => chars(including_char_at(buffer, self.end), false),
            SpanKind::Exclusive if last > first && self.end == buffer.line_start(last) => {
                if in_indent(buffer, self.start) {
                    Target {
                        adjusted: true,
                        ..lines(first, last - 1)
                    }
                } else {
                    chars(buffer.line_end(last - 1), true)
                }
            }
            SpanKind::Exclusive => chars(self.end, false),
        }
    }
}

/// Texto sobre el que actúa un operador.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Region {
    Chars(Range<ByteOffset>),
    Lines { first: usize, last: usize },
}

impl Region {
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Chars(range) if range.is_empty())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Target {
    pub region: Region,
    /// Donde vim deja el cursor antes de aplicar el operador.
    pub start: ByteOffset,
    /// Si la regla `:h exclusive-linewise` movió el final.
    pub adjusted: bool,
}

/// El fin de línea no se incluye: equivale al NUL de vim.
fn including_char_at(buffer: &Buffer, offset: ByteOffset) -> ByteOffset {
    match buffer.char_at(offset) {
        Some(c) if c != '\n' => offset + c.len_utf8(),
        _ => offset,
    }
}
