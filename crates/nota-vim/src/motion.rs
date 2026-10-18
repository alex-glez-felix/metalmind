use nota_buffer::{Buffer, ByteOffset, Goal, Selection};

use crate::Operator;
use crate::cursor::Cursor;
use crate::lines::{Want, first_non_blank, is_blank, last_char};
use crate::scroll::{Place, Viewport};
use crate::span::{Span, SpanKind};
use crate::word;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Left,
    Right,
    Up,
    Down,
    NextWordStart {
        big: bool,
    },
    NextWordEnd {
        big: bool,
    },
    PreviousWordStart {
        big: bool,
    },
    LineStart,
    FirstNonBlank,
    LineEnd,
    DocumentStart,
    DocumentEnd,
    /// `H`, `M` y `L`: una línea de la ventana.
    Window(Place),
}

/// Para qué se resuelve una motion; cambia hasta dónde puede llegar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Purpose {
    /// Mover el cursor en Normal.
    Move,
    /// Mover el extremo de una selección Visual, que puede quedar sobre el
    /// fin de línea.
    Select,
    Operate(Operator),
}

impl Purpose {
    fn reaches_line_end(self) -> bool {
        matches!(self, Self::Select | Self::Operate(_))
    }
}

/// A dónde lleva una motion y cómo la trata un operador.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Destination {
    pub offset: ByteOffset,
    pub goal: Goal,
    pub kind: SpanKind,
}

impl Destination {
    pub const fn new(offset: ByteOffset, kind: SpanKind) -> Self {
        Self {
            offset,
            goal: Goal::None,
            kind,
        }
    }

    pub fn span_from(self, head: ByteOffset) -> Span {
        Span::between(head, self.offset, self.kind)
    }
}

impl Motion {
    /// Como `nv_left` y `nv_right` de vim: en Normal, `h` y `l` fallan si no
    /// pueden moverse.
    pub(crate) fn fails_without_moving(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }

    /// Como en vim, `gg`, `G`, `H`, `M` y `L` dejan el cursor de antes como
    /// la mark de `''`.
    pub(crate) fn is_jump(self) -> bool {
        matches!(
            self,
            Self::DocumentStart | Self::DocumentEnd | Self::Window(_)
        )
    }

    /// `count` es el que escribió el usuario: `gg` y `G` distinguen su ausencia.
    /// `None` si la motion falla (p. ej. `j` en la última línea).
    pub(crate) fn resolve(
        self,
        buffer: &Buffer,
        selection: &Selection,
        count: Option<usize>,
        purpose: Purpose,
        viewport: &Viewport,
    ) -> Option<Destination> {
        let head = selection.head;
        let times = count.unwrap_or(1);
        let line = buffer.line_of(head);
        let destination = match self {
            Self::Left => Destination::new(step_left(buffer, head, times), SpanKind::Exclusive),
            Self::Right => {
                let limit = if purpose.reaches_line_end() {
                    buffer.line_end(line)
                } else {
                    last_char(buffer, line)
                };
                Destination::new(step_right(buffer, head, times, limit), SpanKind::Exclusive)
            }
            Self::Up => vertical(buffer, selection, line_above(line, times)?, purpose),
            Self::Down => vertical(buffer, selection, line_below(buffer, line, times)?, purpose),
            Self::NextWordStart { big } => next_word_start(buffer, head, times, big, purpose),
            Self::NextWordEnd { big } => {
                let mut cursor = Cursor::new(buffer, head);
                word::end(&mut cursor, times, big, false, false);
                off_line_end(cursor, head, SpanKind::Inclusive, purpose)
            }
            Self::PreviousWordStart { big } => {
                let mut cursor = Cursor::new(buffer, head);
                word::backward(&mut cursor, times, big, false);
                Destination::new(cursor.offset, SpanKind::Exclusive)
            }
            Self::LineStart => Destination::new(buffer.line_start(line), SpanKind::Exclusive),
            Self::FirstNonBlank => {
                let offset = first_non_blank(buffer, line).min(last_char(buffer, line));
                Destination::new(offset, SpanKind::Exclusive)
            }
            Self::LineEnd => line_end(buffer, line, times, purpose)?,
            Self::DocumentStart => {
                vertical(buffer, selection, line_number(buffer, count, 0), purpose)
            }
            Self::DocumentEnd => vertical(
                buffer,
                selection,
                line_number(buffer, count, buffer.last_line()),
                purpose,
            ),
            Self::Window(place) => vertical(
                buffer,
                selection,
                viewport.line_at(place, times, buffer),
                purpose,
            ),
        };
        Some(destination)
    }
}

fn step_left(buffer: &Buffer, head: ByteOffset, times: usize) -> ByteOffset {
    let start = buffer.line_start(buffer.line_of(head));
    let mut offset = head;
    for _ in 0..times {
        if offset <= start {
            break;
        }
        offset = buffer.prev_boundary(offset);
    }
    offset
}

fn step_right(buffer: &Buffer, head: ByteOffset, times: usize, limit: ByteOffset) -> ByteOffset {
    let mut offset = head;
    for _ in 0..times {
        if offset >= limit {
            break;
        }
        offset = buffer.next_boundary(offset);
    }
    offset
}

/// Como `cursor_up()` de vim: falla solo si ya está en la primera línea.
fn line_above(line: usize, times: usize) -> Option<usize> {
    line.checked_sub(1)
        .map(|above| above.saturating_sub(times - 1))
}

/// Como `cursor_down()` de vim: falla solo si ya está en la última línea.
fn line_below(buffer: &Buffer, line: usize, times: usize) -> Option<usize> {
    let last = buffer.last_line();
    (line < last).then(|| line.saturating_add(times).min(last))
}

pub(crate) fn vertical(
    buffer: &Buffer,
    selection: &Selection,
    line: usize,
    purpose: Purpose,
) -> Destination {
    let want = Want::of(buffer, selection);
    let offset = if purpose == Purpose::Select {
        want.visual_offset_on(buffer, line)
    } else {
        want.offset_on(buffer, line)
    };
    Destination {
        offset,
        goal: want.goal(),
        kind: SpanKind::Linewise,
    }
}

fn line_number(buffer: &Buffer, count: Option<usize>, default: usize) -> usize {
    count.map_or(default, |n| n.saturating_sub(1).min(buffer.last_line()))
}

/// En Visual `$` llega al fin de línea, que así entra en la selección.
fn line_end(buffer: &Buffer, line: usize, times: usize, purpose: Purpose) -> Option<Destination> {
    let last = buffer.last_line();
    if times > 1 && line >= last {
        return None;
    }
    let target = line.saturating_add(times - 1).min(last);
    let offset = if purpose == Purpose::Select {
        buffer.line_end(target)
    } else {
        last_char(buffer, target)
    };
    Some(Destination {
        offset,
        goal: Goal::LineEnd,
        kind: SpanKind::Inclusive,
    })
}

/// `cw` sobre una palabra se comporta como `ce` (`:h cw`).
fn next_word_start(
    buffer: &Buffer,
    head: ByteOffset,
    times: usize,
    big: bool,
    purpose: Purpose,
) -> Destination {
    let mut cursor = Cursor::new(buffer, head);
    let on_word = cursor.current().is_some_and(|c| !is_blank(c));
    if purpose == Purpose::Operate(Operator::Change) && on_word {
        word::end(&mut cursor, times, big, true, false);
        return off_line_end(cursor, head, SpanKind::Inclusive, purpose);
    }
    let operating = matches!(purpose, Purpose::Operate(_));
    word::forward(&mut cursor, times, big, operating);
    off_line_end(cursor, head, SpanKind::Exclusive, purpose)
}

/// Como `adjust_cursor()` de vim: no dejar el cursor sobre el fin de una
/// línea con texto; si retrocede, el rango pasa a incluir ese carácter. En
/// Visual no se ajusta: el fin de línea es parte de la selección.
fn off_line_end(
    mut cursor: Cursor,
    head: ByteOffset,
    kind: SpanKind,
    purpose: Purpose,
) -> Destination {
    let adjusts = purpose != Purpose::Select;
    if adjusts && cursor.offset > head && cursor.at_line_end() && cursor.one_left() {
        return Destination::new(cursor.offset, SpanKind::Inclusive);
    }
    Destination::new(cursor.offset, kind)
}
