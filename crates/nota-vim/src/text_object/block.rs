use nota_buffer::{Buffer, ByteOffset};

use crate::cursor::{Cursor, Step};
use crate::lines::first_non_blank;
use crate::span::{Span, SpanKind};

/// `i(`, `a(`. Sin bloque alrededor, toma el siguiente, como nvim 0.10.
pub(super) fn select(
    buffer: &Buffer,
    head: ByteOffset,
    count: usize,
    around: bool,
    (open, close): (char, char),
) -> Option<Span> {
    let from = if buffer.char_at(head) == Some(open) {
        buffer.next_boundary(head)
    } else {
        head
    };
    let enclosing = unmatched(buffer, from, open, close, Direction::Backward).is_some();
    let mut start = from;
    for _ in 0..count {
        start = if enclosing {
            unmatched(buffer, start, open, close, Direction::Backward)?
        } else {
            unmatched(buffer, start, open, close, Direction::Forward)?
        };
    }
    let end = unmatched(buffer, start, close, open, Direction::Forward)?;
    if around {
        return Some(Span {
            start,
            end,
            kind: SpanKind::Inclusive,
        });
    }
    Some(inner(buffer, start, end))
}

/// Quita los delimitadores; si el de cierre solo tiene indentación delante,
/// el objeto termina en la línea anterior.
fn inner(buffer: &Buffer, open: ByteOffset, close: ByteOffset) -> Span {
    let mut start = Cursor::new(buffer, open);
    start.incl();
    let mut end = Cursor::new(buffer, close);
    let mut ends_line_above = end.at_line_start();
    end.decl();
    while end.offset < first_non_blank(buffer, end.line()) {
        ends_line_above = true;
        if end.decl() != Step::Moved {
            break;
        }
    }
    if ends_line_above {
        end.incl();
        return Span {
            start: start.offset,
            end: end.offset,
            kind: SpanKind::Exclusive,
        };
    }
    if start.offset <= end.offset {
        Span {
            start: start.offset,
            end: end.offset,
            kind: SpanKind::Inclusive,
        }
    } else {
        Span {
            start: start.offset,
            end: start.offset,
            kind: SpanKind::Exclusive,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    Forward,
    Backward,
}

/// Busca `target` sin pareja, sin mirar el carácter en `from`. `other` anida.
/// Los delimitadores escapados con `\` no cuentan.
fn unmatched(
    buffer: &Buffer,
    from: ByteOffset,
    target: char,
    other: char,
    direction: Direction,
) -> Option<ByteOffset> {
    let mut depth = 0usize;
    let mut offset = from;
    loop {
        let c = match direction {
            Direction::Forward => {
                offset = buffer.next_boundary(offset);
                buffer.char_at(offset)?
            }
            Direction::Backward => {
                let c = buffer.char_before(offset)?;
                offset = buffer.prev_boundary(offset);
                c
            }
        };
        if (c != target && c != other) || escaped(buffer, offset) {
            continue;
        }
        if c == other {
            depth += 1;
        } else if depth == 0 {
            return Some(offset);
        } else {
            depth -= 1;
        }
    }
}

fn escaped(buffer: &Buffer, offset: ByteOffset) -> bool {
    let mut backslashes = 0usize;
    let mut at = offset;
    while buffer.char_before(at) == Some('\\') {
        backslashes += 1;
        at = buffer.prev_boundary(at);
    }
    backslashes % 2 == 1
}
