use nota_buffer::{Buffer, ByteOffset};

use super::Ends;
use crate::cursor::{Class, Cursor, Step};
use crate::span::{Missed, Span, SpanKind};
use crate::word::{back_in_line, backward, backward_end, end, forward};

/// `iw`, `aw`, `iW`, `aW` con un operador pendiente. Como `current_word`, si
/// falla deja el cursor donde llegó buscando.
pub(super) fn resolve(
    buffer: &Buffer,
    head: ByteOffset,
    count: usize,
    big: bool,
    around: bool,
) -> Result<Span, Missed> {
    let mut ends = Ends::at(head);
    let inclusive = pick(buffer, &mut ends, false, count, big, around)
        .ok_or_else(|| Missed::at(buffer, ends.head))?;
    let kind = if inclusive {
        SpanKind::Inclusive
    } else {
        SpanKind::Exclusive
    };
    Ok(Span {
        start: ends.anchor,
        end: ends.head,
        kind,
    })
}

/// En Visual; `false` si falló.
pub(super) fn select(
    buffer: &Buffer,
    ends: &mut Ends,
    count: usize,
    big: bool,
    around: bool,
) -> bool {
    pick(buffer, ends, true, count, big, around).is_some()
}

/// `current_word` de vim. Devuelve si el final es inclusivo; si falla puede
/// dejar movido `ends.head`, como vim deja el cursor.
fn pick(
    buffer: &Buffer,
    ends: &mut Ends,
    visual: bool,
    mut count: usize,
    big: bool,
    around: bool,
) -> Option<bool> {
    let mut cursor = Cursor::new(buffer, ends.head);
    let moved = |cursor: &Cursor, ends: &mut Ends| ends.head = cursor.offset;
    let mut include_white = false;
    if !visual || ends.anchor == ends.head {
        back_in_line(&mut cursor, big);
        let start = cursor.offset;
        if (cursor.class(big) == Class::Blank) == around {
            let found = end(&mut cursor, 1, big, true, true);
            moved(&cursor, ends);
            if !found {
                return None;
            }
        } else {
            forward(&mut cursor, 1, big, true);
            if cursor.at_line_start() {
                cursor.decl();
            } else {
                cursor.one_left();
            }
            include_white = around;
        }
        ends.anchor = start;
        count -= 1;
    }
    let extended = extend(&mut cursor, ends.anchor, visual, count, big, around);
    moved(&cursor, ends);
    let inclusive = extended?;
    if include_white
        && (cursor.class(big) != Class::Blank || (cursor.at_line_start() && !inclusive))
    {
        ends.anchor = with_leading_white(buffer, ends.anchor, big);
    }
    Some(inclusive)
}

/// Agrega `count` objetos más, hacia atrás si en Visual el cursor está
/// antes del otro extremo; devuelve si el final es inclusivo.
fn extend(
    cursor: &mut Cursor,
    anchor: ByteOffset,
    visual: bool,
    count: usize,
    big: bool,
    around: bool,
) -> Option<bool> {
    let mut inclusive = true;
    for remaining in (1..=count).rev() {
        inclusive = true;
        if visual && cursor.offset < anchor {
            if cursor.decl() == Step::Edge {
                return None;
            }
            if around == (cursor.class(big) == Class::Blank) {
                if !backward(cursor, 1, big, true) {
                    return None;
                }
            } else {
                if !backward_end(cursor, 1, big, true) {
                    return None;
                }
                cursor.incl();
            }
            continue;
        }
        if cursor.incl() == Step::Edge {
            return None;
        }
        if around == (cursor.class(big) == Class::Blank) {
            if !end(cursor, 1, big, true, true) {
                return None;
            }
            continue;
        }
        if !forward(cursor, 1, big, true) && remaining > 1 {
            return None;
        }
        if !cursor.one_left() {
            inclusive = false;
        }
    }
    Some(inclusive)
}

/// Sin blancos detrás, `aw` toma los de delante, salvo la indentación.
fn with_leading_white(buffer: &Buffer, start: ByteOffset, big: bool) -> ByteOffset {
    let mut probe = Cursor::new(buffer, start);
    if !probe.one_left() {
        return start;
    }
    back_in_line(&mut probe, big);
    if probe.class(big) == Class::Blank && !probe.at_line_start() {
        probe.offset
    } else {
        start
    }
}
