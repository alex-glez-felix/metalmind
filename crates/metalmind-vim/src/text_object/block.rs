use metalmind_buffer::{Buffer, ByteOffset};

use super::Ends;
use crate::cursor::{Cursor, Step};
use crate::lines::first_non_blank;
use crate::span::{Span, SpanKind};

/// `i(`, `a(` con un operador pendiente. Sin bloque alrededor, toma el
/// siguiente, como nvim.
pub(super) fn resolve(
    buffer: &Buffer,
    head: ByteOffset,
    count: usize,
    around: bool,
    pair: (char, char),
) -> Option<Span> {
    let found = pick(buffer, Ends::at(head), false, count, around, pair)?;
    if around {
        return Some(Span {
            start: found.ends.anchor,
            end: found.ends.head,
            kind: SpanKind::Inclusive,
        });
    }
    let (start, end) = (found.ends.anchor, found.ends.head);
    let span = if found.ends_line_above {
        let mut end = Cursor::new(buffer, end);
        end.incl();
        Span::between(start, end.offset, SpanKind::Exclusive)
    } else if start <= end {
        Span::between(start, end, SpanKind::Inclusive)
    } else {
        Span::between(start, start, SpanKind::Exclusive)
    };
    Some(span)
}

/// En Visual; `false` si falló, y entonces no mueve nada.
pub(super) fn select(
    buffer: &Buffer,
    ends: &mut Ends,
    count: usize,
    around: bool,
    pair: (char, char),
) -> bool {
    let Some(found) = pick(buffer, *ends, true, count, around, pair) else {
        return false;
    };
    let mut head = Cursor::new(buffer, found.ends.head);
    if found.ends_line_above && !head.at_line_end() {
        head.inc();
    }
    *ends = Ends {
        anchor: found.ends.anchor,
        head: head.offset,
    };
    true
}

/// Un bloque ya encontrado: `anchor` y `head` son el inicio y el final.
struct Found {
    ends: Ends,
    /// Sin los delimitadores, el de cierre solo tenía indentación delante y
    /// el objeto termina en la línea anterior.
    ends_line_above: bool,
}

/// `current_block` de vim. En Visual, con algo ya seleccionado, busca desde
/// el extremo de abajo; si el interior no agranda la selección, pasa al
/// bloque de afuera.
fn pick(
    buffer: &Buffer,
    ends: Ends,
    visual: bool,
    count: usize,
    around: bool,
    (open, close): (char, char),
) -> Option<Found> {
    let (mut old_start, mut old_end) = (ends.head, ends.head);
    let mut from = ends.head;
    if !visual || ends.anchor == ends.head {
        if buffer.char_at(from) == Some(open) {
            from = buffer.next_boundary(from);
        }
    } else if ends.anchor < ends.head {
        old_start = ends.anchor;
        from = ends.anchor;
    } else {
        old_end = ends.anchor;
    }
    let enclosing = unmatched(buffer, from, open, close, Direction::Backward).is_some();
    let direction = if enclosing {
        Direction::Backward
    } else {
        Direction::Forward
    };
    let mut start = from;
    for _ in 0..count {
        start = unmatched(buffer, start, open, close, direction)?;
    }
    let mut end = unmatched(buffer, start, close, open, Direction::Forward)?;
    if around {
        return Some(Found {
            ends: Ends {
                anchor: start,
                head: end,
            },
            ends_line_above: false,
        });
    }
    loop {
        let (inner_start, inner_end, ends_line_above) = inner(buffer, start, end);
        if visual && inner_start == end {
            return None;
        }
        let grows = inner_start < old_start || old_end < inner_end;
        if !visual || grows || inner_start == inner_end {
            return Some(Found {
                ends: Ends {
                    anchor: inner_start,
                    head: inner_end,
                },
                ends_line_above,
            });
        }
        let mut before = Cursor::new(buffer, old_start);
        before.decl();
        start = unmatched(buffer, before.offset, open, close, Direction::Backward)?;
        end = unmatched(buffer, start, close, open, Direction::Forward)?;
    }
}

/// Quita los delimitadores; si el de cierre solo tiene indentación delante,
/// el objeto termina en la línea anterior.
fn inner(buffer: &Buffer, open: ByteOffset, close: ByteOffset) -> (ByteOffset, ByteOffset, bool) {
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
    (start.offset, end.offset, ends_line_above)
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
