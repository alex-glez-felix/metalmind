//! Selecciones de Visual: qué texto abarcan, su tamaño para `.` y
//! `[count]v`, y lo que guarda `gv`.

use nota_buffer::{Buffer, ByteOffset, Goal, Selection, Selections};

use crate::VisualKind;
use crate::span::{Region, Target};

/// El texto que abarca una selección Visual. Vim incluye el carácter bajo
/// los dos extremos, y el salto de línea si el final está sobre él.
pub(crate) fn target(buffer: &Buffer, selection: &Selection, kind: VisualKind) -> Target {
    let (start, end) = (selection.start(), selection.end());
    let (region, start) = match kind {
        VisualKind::Char => (Region::Chars(start..buffer.next_boundary(end)), start),
        VisualKind::Line => {
            let first = buffer.line_of(start);
            let last = buffer.line_of(end);
            (Region::Lines { first, last }, buffer.line_start(first))
        }
    };
    Target {
        region,
        start,
        adjusted: false,
        visual: true,
    }
}

/// Sobre qué actúa un operador en Visual.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Area {
    /// Cada selección, de `anchor` a `head`.
    Selection(VisualKind),
    /// Lo que repite `.`: la misma cantidad de texto desde el cursor.
    Sized(VisualSize),
}

impl Area {
    pub fn target(self, buffer: &Buffer, selection: &Selection) -> Target {
        match self {
            Self::Selection(kind) => target(buffer, selection, kind),
            Self::Sized(size) => {
                let sized = size.select_from(buffer, selection.head, 1);
                target(buffer, &sized, size.kind)
            }
        }
    }
}

/// El tamaño de la última selección sobre la que se operó, como lo guarda
/// vim para `.` y `[count]v` (`resel_VIsual_*`). Las columnas son de
/// caracteres; vim mide columnas de pantalla.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct VisualSize {
    kind: VisualKind,
    lines: usize,
    reach: Reach,
}

/// Hasta dónde llega la selección en su última línea.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reach {
    /// En una sola línea: cuántos caracteres abarca.
    Width(usize),
    /// En varias líneas: la columna del final.
    Column(usize),
    /// Hasta el fin de línea, tras `$`.
    LineEnd,
}

impl VisualSize {
    pub fn of(buffer: &Buffer, selection: &Selection, kind: VisualKind) -> Self {
        let (start, end) = (selection.start(), selection.end());
        let lines = buffer.line_of(end) - buffer.line_of(start) + 1;
        let end_column = buffer.column_of(end);
        let reach = if selection.goal == Goal::LineEnd {
            Reach::LineEnd
        } else if lines == 1 {
            Reach::Width(end_column - buffer.column_of(start) + 1)
        } else {
            Reach::Column(end_column)
        };
        Self { kind, lines, reach }
    }

    pub fn kind(self) -> VisualKind {
        self.kind
    }

    /// Una selección desde `head` con este tamaño multiplicado por `times`.
    /// Como en vim, solo se multiplican las líneas de una selección por
    /// líneas o de varias, y el ancho de una de una sola línea.
    pub fn select_from(self, buffer: &Buffer, head: ByteOffset, times: usize) -> Selection {
        let line = buffer.line_of(head);
        let end_line = if self.kind == VisualKind::Line || self.lines > 1 {
            let lines = self.lines.saturating_mul(times);
            line.saturating_add(lines - 1).min(buffer.last_line())
        } else {
            line
        };
        let column = buffer.column_of(head);
        let (end, goal) = match (self.reach, self.kind) {
            (Reach::LineEnd, _) => (buffer.line_end(end_line), Goal::LineEnd),
            (Reach::Width(width), VisualKind::Char) => {
                let column = column + width.saturating_mul(times) - 1;
                (
                    buffer.offset_at_column(end_line, column),
                    Goal::Column(column),
                )
            }
            (Reach::Column(column), VisualKind::Char) => (
                buffer.offset_at_column(end_line, column),
                Goal::Column(column),
            ),
            (_, VisualKind::Line) => (buffer.offset_at_column(end_line, column), Goal::None),
        };
        Selection {
            anchor: head,
            head: end,
            goal,
        }
    }
}

/// Lo que `gv` vuelve a seleccionar, como las marcas `'<` y `'>` de vim:
/// línea y byte dentro de la línea, sin seguir las ediciones posteriores.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LastVisual {
    kind: VisualKind,
    ends: Vec<Ends>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Ends {
    anchor: Mark,
    head: Mark,
    goal: Goal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Mark {
    line: usize,
    byte: usize,
}

impl Mark {
    fn of(buffer: &Buffer, offset: ByteOffset) -> Self {
        let line = buffer.line_of(offset);
        Self {
            line,
            byte: offset.get() - buffer.line_start(line).get(),
        }
    }

    /// Lo más cerca que se puede en el texto de ahora, sin pasar del fin de
    /// su línea y sin partir un carácter (`check_cursor` de vim).
    fn place(self, buffer: &Buffer) -> ByteOffset {
        let line = self.line.min(buffer.last_line());
        let mut offset = (buffer.line_start(line) + self.byte).min(buffer.line_end(line));
        while !buffer.is_char_boundary(offset) {
            offset = offset.saturating_sub(1);
        }
        offset
    }
}

impl LastVisual {
    pub fn save(buffer: &Buffer, selections: &Selections, kind: VisualKind) -> Self {
        let ends = selections
            .iter()
            .map(|selection| Ends {
                anchor: Mark::of(buffer, selection.anchor),
                head: Mark::of(buffer, selection.head),
                goal: selection.goal,
            })
            .collect();
        Self { kind, ends }
    }

    pub fn kind(&self) -> VisualKind {
        self.kind
    }

    pub fn restore(&self, buffer: &Buffer) -> Option<Selections> {
        Selections::new(self.ends.iter().map(|ends| Selection {
            anchor: ends.anchor.place(buffer),
            head: ends.head.place(buffer),
            goal: ends.goal,
        }))
    }
}
