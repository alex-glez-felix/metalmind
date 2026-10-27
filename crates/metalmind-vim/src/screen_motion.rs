//! `gj`, `gk`, `g0`, `g^`, `gm` y `g$`: van por las filas de la pantalla.
//! Con `wrap`, por las de una línea partida, como `nv_screengo` y `nv_g_cmd`
//! de vim; sin él, `gj` y `gk` son `j` y `k` y las demás miden lo que se ve
//! de la línea a lo ancho. Sin pantalla (un motor sin UI), la línea entera
//! es una fila.

use std::sync::Arc;

use metalmind_buffer::{Buffer, ByteOffset, Goal, Selection};
use metalmind_syntax::{Screen, ScreenLine};

use crate::Mode;
use crate::PaneState;
use crate::columns::Columns;
use crate::lines::{clamp_normal, is_blank, last_char, tab_end};
use crate::markdown::LazyStyles;
use crate::motion::{Destination, Motion, Purpose};
use crate::scroll::{f32_of, units};
use crate::span::SpanKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenMotion {
    /// `gj`.
    Down,
    /// `gk`.
    Up,
    /// `g0`.
    RowStart,
    /// `g^`.
    RowFirstNonBlank,
    /// `gm`: la mitad del ancho de la pantalla.
    RowMiddle,
    /// `g$`.
    RowEnd,
}

/// Lo que una motion de pantalla mira: el documento y cómo se ve.
struct Rows<'a, 's> {
    buffer: &'a Buffer,
    styles: &'a LazyStyles<'s>,
    screen: &'a Screen,
    columns: Columns,
}

impl Rows<'_, '_> {
    fn line(&self, line: usize) -> Arc<ScreenLine> {
        let style = self.styles.line(self.buffer, line);
        self.screen
            .line(&self.buffer.line_text(line), style.as_ref())
    }

    /// La columna de vim (con tabs y caracteres anchos) donde empieza la
    /// fila `row` de `line`.
    fn row_column(&self, line: usize, shown: &ScreenLine, row: usize) -> usize {
        let start = shown.row(row).start;
        self.columns
            .span_of(self.buffer, self.buffer.line_start(line) + start)
            .0
    }

    /// Cuántas columnas de texto caben en una fila.
    fn width_columns(&self) -> usize {
        let columns = (self.screen.width / self.screen.column.max(1.0)).floor();
        units(columns).max(1)
    }
}

impl ScreenMotion {
    pub(crate) fn resolve(
        self,
        buffer: &Buffer,
        selection: &Selection,
        count: Option<usize>,
        purpose: Purpose,
        pane: &PaneState,
        styles: &LazyStyles,
    ) -> Option<Destination> {
        let Some(screen) = pane.screen.as_ref() else {
            return self.as_line(buffer, selection, count, purpose, pane, styles);
        };
        let rows = Rows {
            buffer,
            styles,
            screen,
            columns: pane.viewport.columns(),
        };
        let times = count.unwrap_or(1);
        let line = buffer.line_of(selection.head);
        let byte = selection.head.get() - buffer.line_start(line).get();
        match (self, screen.wrap) {
            (Self::Down | Self::Up, false) => {
                self.as_line(buffer, selection, count, purpose, pane, styles)
            }
            (Self::Down | Self::Up, true) => {
                screen_go(&rows, selection, self == Self::Down, times, purpose)
            }
            (Self::RowEnd, true) if times > 1 => {
                let at_end = Selection {
                    goal: Goal::LineEnd,
                    ..*selection
                };
                screen_go(&rows, &at_end, true, times - 1, purpose)
            }
            (_, true) => {
                let shown = rows.line(line);
                let range = shown.row(shown.row_of(byte));
                let row = (
                    range.start,
                    last_of_row(&rows, line, &shown, range.end, purpose),
                );
                Some(self.in_row(&rows, line, &shown, row, purpose))
            }
            (_, false) => {
                let line = match self {
                    Self::RowEnd if times > 1 => {
                        line.saturating_add(times - 1).min(buffer.last_line())
                    }
                    _ => line,
                };
                let shown = rows.line(line);
                Some(self.in_view(&rows, line, &shown, pane.viewport.left(), purpose))
            }
        }
    }

    /// Sin pantalla, o `gj` y `gk` sin `wrap`: lo mismo por líneas.
    fn as_line(
        self,
        buffer: &Buffer,
        selection: &Selection,
        count: Option<usize>,
        purpose: Purpose,
        pane: &PaneState,
        styles: &LazyStyles,
    ) -> Option<Destination> {
        let motion = match self {
            Self::Down => Motion::Down,
            Self::Up => Motion::Up,
            Self::RowStart => Motion::LineStart,
            Self::RowFirstNonBlank => Motion::FirstNonBlank,
            Self::RowMiddle | Self::RowEnd => Motion::LineEnd,
        };
        motion.resolve(buffer, selection, count, purpose, pane, styles)
    }

    /// `g0`, `g^`, `gm` o `g$` en la fila que va de `row.0` hasta su último
    /// carácter `row.1`, con `wrap`.
    fn in_row(
        self,
        rows: &Rows<'_, '_>,
        line: usize,
        shown: &ScreenLine,
        (start, last): (usize, ByteOffset),
        purpose: Purpose,
    ) -> Destination {
        let buffer = rows.buffer;
        let line_start = buffer.line_start(line);
        let offset = match self {
            Self::RowStart | Self::Down | Self::Up => line_start + start,
            Self::RowFirstNonBlank => skip_blanks(buffer, line_start + start, line),
            Self::RowMiddle => {
                let row = shown.row_of(start);
                let byte = shown.byte_at(row, rows.screen.width / 2.0);
                (line_start + byte).min(last)
            }
            Self::RowEnd => last,
        };
        self.landing(rows, offset, purpose)
    }

    /// `g0`, `g^`, `gm` o `g$` sin `wrap`, sobre lo que se ve de `line`
    /// desde `left`.
    fn in_view(
        self,
        rows: &Rows<'_, '_>,
        line: usize,
        shown: &ScreenLine,
        left: usize,
        purpose: Purpose,
    ) -> Destination {
        let buffer = rows.buffer;
        let advances = shown.advances();
        let (from, width) = (f32_of(left), rows.screen.width);
        let at = |x: f32| buffer.line_start(line) + advances.byte_at(x);
        let offset = match self {
            Self::RowStart | Self::Down | Self::Up => at(from),
            Self::RowFirstNonBlank => skip_blanks(buffer, at(from), line),
            Self::RowMiddle => at(from + (width / 2.0).floor()),
            Self::RowEnd => {
                let edge = from + width;
                let offset = at(edge - rows.screen.column / 2.0);
                let byte = offset.get() - buffer.line_start(line).get();
                if byte < advances.len() && advances.end(byte) > edge && byte > 0 {
                    buffer.prev_boundary(offset)
                } else {
                    offset
                }
            }
        };
        self.landing(rows, offset, purpose)
    }

    /// Como lo deja vim: `g$` es inclusiva y fija la columna donde quedó;
    /// las demás son exclusivas y la vuelven a medir.
    fn landing(self, rows: &Rows<'_, '_>, offset: ByteOffset, purpose: Purpose) -> Destination {
        let offset = if purpose == Purpose::Select {
            offset
        } else {
            clamp_normal(rows.buffer, offset)
        };
        if self == Self::RowEnd {
            let column = rows.columns.span_of(rows.buffer, offset).0;
            return Destination {
                offset,
                goal: Goal::Column(column),
                kind: SpanKind::Inclusive,
            };
        }
        Destination::new(offset, SpanKind::Exclusive)
    }
}

/// `gj` o `gk` `times` filas, con `wrap`, conservando la columna dentro de
/// la fila como `nv_screengo` de vim: el `curswant` es la columna en la
/// línea, y una fila más abajo en la misma línea es la misma columna desde
/// donde empieza esa fila. Tras `$` (`Goal::LineEnd`) va al final de cada
/// fila. Falla si no puede moverse, y con un operador si no llega.
fn screen_go(
    rows: &Rows<'_, '_>,
    selection: &Selection,
    forward: bool,
    times: usize,
    purpose: Purpose,
) -> Option<Destination> {
    let buffer = rows.buffer;
    let head = selection.head;
    let mut line = buffer.line_of(head);
    let mut shown = rows.line(line);
    let mut row = shown.row_of(head.get() - buffer.line_start(line).get());
    let at_end = selection.goal == Goal::LineEnd;
    let mode = if purpose == Purpose::Select {
        Mode::Visual(crate::VisualKind::Char)
    } else {
        Mode::Normal
    };
    let want = match selection.goal {
        Goal::Column(column) => column,
        Goal::None | Goal::LineEnd => {
            rows.columns
                .cursor_at(buffer, head, tab_end(mode, selection))
        }
    };
    let column = want
        .saturating_sub(rows.row_column(line, &shown, row))
        .min(rows.width_columns() - 1);
    let mut moved = 0;
    while moved < times {
        if forward && row + 1 < shown.rows() {
            row += 1;
        } else if forward && line < buffer.last_line() {
            line += 1;
            shown = rows.line(line);
            row = 0;
        } else if !forward && row > 0 {
            row -= 1;
        } else if !forward && line > 0 {
            line -= 1;
            shown = rows.line(line);
            row = shown.rows() - 1;
        } else {
            break;
        }
        moved += 1;
    }
    let operating = matches!(purpose, Purpose::Operate(_));
    if moved == 0 || (operating && moved < times) {
        return None;
    }
    let range = shown.row(row);
    let last = last_of_row(rows, line, &shown, range.end, purpose);
    let start_column = rows.row_column(line, &shown, row);
    let offset = if at_end {
        last
    } else {
        let past_end = purpose == Purpose::Select;
        rows.columns
            .offset_at(buffer, line, start_column + column, past_end)
            .min(last)
    };
    Some(Destination {
        offset,
        goal: if at_end {
            Goal::LineEnd
        } else {
            Goal::Column(start_column + column)
        },
        kind: if at_end {
            SpanKind::Inclusive
        } else {
            SpanKind::Exclusive
        },
    })
}

/// El último carácter de la fila que termina en `end`; en la última, el de
/// la línea o, en Visual, su fin.
fn last_of_row(
    rows: &Rows<'_, '_>,
    line: usize,
    shown: &ScreenLine,
    end: usize,
    purpose: Purpose,
) -> ByteOffset {
    let buffer = rows.buffer;
    if end < shown.len() {
        return buffer.prev_boundary(buffer.line_start(line) + end);
    }
    if purpose == Purpose::Select {
        buffer.line_end(line)
    } else {
        last_char(buffer, line)
    }
}

/// Desde `offset`, el primer carácter que no es blanco, sin pasar del
/// último de la línea.
fn skip_blanks(buffer: &Buffer, mut offset: ByteOffset, line: usize) -> ByteOffset {
    let last = last_char(buffer, line);
    while offset < last && buffer.char_at(offset).is_some_and(is_blank) {
        offset = buffer.next_boundary(offset);
    }
    offset
}
