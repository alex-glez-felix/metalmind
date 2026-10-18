use nota_buffer::{Buffer, ByteOffset};

use crate::lines::first_non_blank;
use crate::span::{Span, SpanKind};

/// `ip`, `ap`. Un párrafo es un bloque de líneas con texto o uno de líneas
/// en blanco; `ap` suma las en blanco de después, o las de antes si no hay.
pub(super) fn select(
    buffer: &Buffer,
    head: ByteOffset,
    count: usize,
    around: bool,
) -> Option<Span> {
    let lines = buffer.line_count();
    let blank = |line| first_non_blank(buffer, line) == buffer.line_end(line);
    let blank_in_front = blank(buffer.line_of(head));
    let mut start = buffer.line_of(head);
    while start > 0 && blank(start - 1) == blank_in_front {
        start -= 1;
    }
    // Primera línea fuera del objeto.
    let mut end = start;
    while end < lines && blank(end) {
        end += 1;
    }
    let skip = |mut line: usize, want_blank: bool| {
        while line < lines && blank(line) == want_blank {
            line += 1;
        }
        line
    };
    let mut remaining = count - usize::from(!around && blank_in_front);
    while remaining > 0 {
        remaining -= 1;
        if end == lines {
            return None;
        }
        let next_is_blank = !around && blank(end);
        if around || !next_is_blank {
            end = skip(end + 1, false);
        }
        if remaining == 0 && blank_in_front && around {
            break;
        }
        if around || next_is_blank {
            end = skip(end, true);
        }
    }
    if around && !blank_in_front && !blank(end - 1) {
        while start > 0 && blank(start - 1) {
            start -= 1;
        }
    }
    Some(Span {
        start: buffer.line_start(start),
        end: buffer.line_start(end - 1),
        kind: SpanKind::Linewise,
    })
}
