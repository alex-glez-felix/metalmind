use nota_buffer::{Buffer, ByteOffset};

use crate::cursor::{Class, Cursor, Step};
use crate::span::{Missed, Span, SpanKind};
use crate::word::{back_in_line, end, forward};

/// `iw`, `aw`, `iW`, `aW`. Como `current_word`, si falla deja el cursor
/// donde llegó buscando.
pub(super) fn select(
    buffer: &Buffer,
    head: ByteOffset,
    count: usize,
    big: bool,
    around: bool,
) -> Result<Span, Missed> {
    let mut cursor = Cursor::new(buffer, head);
    back_in_line(&mut cursor, big);
    let mut start = cursor.offset;
    let mut include_white = false;
    if (cursor.class(big) == Class::Blank) == around {
        if !end(&mut cursor, 1, big, true, true) {
            return Err(Missed::at(buffer, cursor.offset));
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
    let inclusive =
        extend(&mut cursor, count, big, around).ok_or_else(|| Missed::at(buffer, cursor.offset))?;
    if include_white
        && (cursor.class(big) != Class::Blank || (cursor.at_line_start() && !inclusive))
    {
        start = with_leading_white(buffer, start, big);
    }
    let kind = if inclusive {
        SpanKind::Inclusive
    } else {
        SpanKind::Exclusive
    };
    Ok(Span {
        start,
        end: cursor.offset,
        kind,
    })
}

/// Agrega `count - 1` objetos más; devuelve si el final es inclusivo, o
/// `None` con `cursor` donde se quedó.
fn extend(cursor: &mut Cursor, count: usize, big: bool, around: bool) -> Option<bool> {
    let mut inclusive = true;
    for remaining in (1..count).rev() {
        inclusive = true;
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
