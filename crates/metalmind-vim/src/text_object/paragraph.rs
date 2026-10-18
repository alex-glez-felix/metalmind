use metalmind_buffer::{Buffer, ByteOffset};

use super::Ends;
use crate::VisualKind;
use crate::lines::first_non_blank;
use crate::span::{Span, SpanKind};

/// `ip`, `ap` con un operador pendiente. Un párrafo es un bloque de líneas
/// con texto o uno de líneas en blanco; `ap` suma las en blanco de después,
/// o las de antes si no hay.
pub(super) fn resolve(
    buffer: &Buffer,
    head: ByteOffset,
    count: usize,
    around: bool,
) -> Option<Span> {
    let (first, last) = paragraph(buffer, buffer.line_of(head), count, around)?;
    Some(Span {
        start: buffer.line_start(first),
        end: buffer.line_start(last),
        kind: SpanKind::Linewise,
    })
}

/// En Visual devuelve el modo nuevo. Con varias líneas seleccionadas, o si
/// el párrafo no agranda una selección por líneas, la extiende hacia el
/// lado del cursor sin cambiar el modo; si no, selecciona el párrafo por
/// líneas.
pub(super) fn select(
    buffer: &Buffer,
    ends: &mut Ends,
    kind: VisualKind,
    count: usize,
    around: bool,
) -> VisualKind {
    let line = buffer.line_of(ends.head);
    let anchor_line = buffer.line_of(ends.anchor);
    if anchor_line != line {
        extend(buffer, ends, count, around);
        return kind;
    }
    let Some((first, last)) = paragraph(buffer, line, count, around) else {
        return kind;
    };
    if kind == VisualKind::Line && first == line {
        extend(buffer, ends, count, around);
        return kind;
    }
    if anchor_line != first {
        ends.anchor = buffer.line_start(first);
    }
    ends.head = buffer.line_start(last);
    VisualKind::Line
}

/// Líneas de un párrafo contadas desde 1, como en `current_par` de vim.
struct Lines<'a> {
    buffer: &'a Buffer,
    count: usize,
}

impl Lines<'_> {
    fn white(&self, number: usize) -> bool {
        let line = number - 1;
        first_non_blank(self.buffer, line) == self.buffer.line_end(line)
    }
}

/// La primera y la última línea del objeto, desde 0.
fn paragraph(buffer: &Buffer, line: usize, count: usize, around: bool) -> Option<(usize, usize)> {
    let lines = Lines {
        buffer,
        count: buffer.line_count(),
    };
    let mut start = line + 1;
    let white_in_front = lines.white(start);
    while start > 1 && lines.white(start - 1) == white_in_front {
        start -= 1;
    }
    let mut end = start;
    while end <= lines.count && lines.white(end) {
        end += 1;
    }
    end -= 1;
    let mut remaining = count - usize::from(!around && white_in_front);
    while remaining > 0 {
        remaining -= 1;
        if end == lines.count {
            return None;
        }
        let do_white = !around && lines.white(end + 1);
        if around || !do_white {
            end += 1;
            while end < lines.count && !lines.white(end + 1) {
                end += 1;
            }
        }
        if remaining == 0 && white_in_front && around {
            break;
        }
        if around || do_white {
            while end < lines.count && lines.white(end + 1) {
                end += 1;
            }
        }
    }
    if around && !white_in_front && !lines.white(end) {
        while start > 1 && lines.white(start - 1) {
            start -= 1;
        }
    }
    Some((start - 1, end - 1))
}

/// Mueve el cursor `count` párrafos (o bloques de líneas en blanco) hacia
/// donde apunta la selección, al inicio de la línea. Como en vim, si choca
/// con el borde del texto deja el cursor donde llegó.
fn extend(buffer: &Buffer, ends: &mut Ends, count: usize, around: bool) {
    let lines = Lines {
        buffer,
        count: buffer.line_count(),
    };
    let mut start = buffer.line_of(ends.head) + 1;
    let backward = start < buffer.line_of(ends.anchor) + 1;
    let edge = if backward { 1 } else { lines.count };
    let next = |number: usize| if backward { number - 1 } else { number + 1 };
    let back = |number: usize| if backward { number + 1 } else { number - 1 };
    for _ in 0..count {
        if start == edge {
            break;
        }
        let mut previous_white = None;
        for _ in 0..2 {
            start = next(start);
            let start_white = lines.white(start);
            if previous_white == Some(start_white) {
                start = back(start);
                break;
            }
            while start != edge && lines.white(next(start)) == start_white {
                start = next(start);
            }
            if !around || start == edge {
                break;
            }
            previous_white = Some(start_white);
        }
    }
    ends.head = buffer.line_start(start - 1);
}
