//! Selecciones de Visual: qué texto abarcan, su tamaño para `.` y
//! `[count]v`, y lo que guarda `gv`.

use metalmind_buffer::{Buffer, ByteOffset, Goal, Selection, Selections};

use crate::VisualKind;
use crate::block::BlockSize;
use crate::columns::Columns;
use crate::span::{Extent, Region};

/// Lo que reusan `[count]v`, `[count]V` y `[count]ctrl-v`: el tamaño de la
/// última selección sobre la que se operó.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Reselect {
    Range(VisualSize),
    Block(BlockSize),
}

/// El texto que abarca una selección Visual. Vim incluye el carácter bajo
/// los dos extremos, y el salto de línea si el final está sobre él.
pub(crate) fn target(buffer: &Buffer, selection: &Selection, kind: VisualKind) -> Extent {
    let (start, end) = (selection.start(), selection.end());
    let (region, start) = match kind {
        // Los operadores de un bloque no pasan por aquí (ver `crate::block`).
        VisualKind::Char | VisualKind::Block => {
            (Region::Chars(start..buffer.next_boundary(end)), start)
        }
        VisualKind::Line => {
            let first = buffer.line_of(start);
            let last = buffer.line_of(end);
            (Region::Lines { first, last }, buffer.line_start(first))
        }
    };
    Extent {
        region,
        start,
        adjusted: false,
        visual: true,
        nested: false,
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
    pub fn target(self, buffer: &Buffer, selection: &Selection, columns: Columns) -> Extent {
        match self {
            Self::Selection(kind) => target(buffer, selection, kind),
            Self::Sized(size) => {
                let sized = size.select_from(buffer, selection.head, 1, columns);
                target(buffer, &sized, size.kind())
            }
        }
    }
}

/// El tamaño de la última selección por caracteres o por líneas sobre la
/// que se operó, como lo guarda vim para `.` y `[count]v`
/// (`resel_VIsual_*`), en columnas de pantalla. El de un bloque es un
/// [`BlockSize`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct VisualSize {
    linewise: bool,
    lines: usize,
    reach: Reach,
}

/// Hasta dónde llega la selección en su última línea.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reach {
    /// En una sola línea: cuántas columnas abarca.
    Width(usize),
    /// En varias líneas: la última columna del carácter del final.
    Column(usize),
    /// Hasta el fin de línea, tras `$`.
    LineEnd,
}

impl VisualSize {
    pub fn of(buffer: &Buffer, selection: &Selection, kind: VisualKind, columns: Columns) -> Self {
        let (start, end) = (selection.start(), selection.end());
        let lines = buffer.line_of(end) - buffer.line_of(start) + 1;
        let (_, end_column) = columns.span_of(buffer, end);
        let reach = if selection.goal == Goal::LineEnd {
            Reach::LineEnd
        } else if lines == 1 {
            let (start_column, _) = columns.span_of(buffer, start);
            Reach::Width(end_column - start_column + 1)
        } else {
            Reach::Column(end_column)
        };
        Self {
            linewise: kind == VisualKind::Line,
            lines,
            reach,
        }
    }

    pub fn kind(self) -> VisualKind {
        if self.linewise {
            VisualKind::Line
        } else {
            VisualKind::Char
        }
    }

    /// Una selección desde `head` con este tamaño multiplicado por `times`.
    /// Como en vim, solo se multiplican las líneas de una selección por
    /// líneas o de varias, y el ancho de una de una sola línea, que se mide
    /// desde donde se ve el cursor.
    pub fn select_from(
        self,
        buffer: &Buffer,
        head: ByteOffset,
        times: usize,
        columns: Columns,
    ) -> Selection {
        let line = buffer.line_of(head);
        let end_line = if self.linewise || self.lines > 1 {
            let lines = self.lines.saturating_mul(times);
            line.saturating_add(lines - 1).min(buffer.last_line())
        } else {
            line
        };
        let column = columns.cursor_at(buffer, head, true);
        let at = |column| columns.offset_at(buffer, end_line, column, true);
        let (end, goal) = match (self.reach, self.linewise) {
            (Reach::LineEnd, _) => (buffer.line_end(end_line), Goal::LineEnd),
            (Reach::Width(width), false) => {
                let column = column + width.saturating_mul(times) - 1;
                (at(column), Goal::Column(column))
            }
            (Reach::Column(column), false) => (at(column), Goal::Column(column)),
            (_, true) => (at(column), Goal::None),
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

    /// Las líneas de `'<` y `'>`: las de los extremos de la primera
    /// selección, sin pasar del final.
    pub fn lines(&self, buffer: &Buffer) -> Option<(usize, usize)> {
        let last = buffer.last_line();
        let ends = self.ends.first()?;
        let (a, b) = (ends.anchor.line.min(last), ends.head.line.min(last));
        Some((a.min(b), a.max(b)))
    }

    pub fn restore(&self, buffer: &Buffer) -> Option<Selections> {
        Selections::new(self.ends.iter().map(|ends| Selection {
            anchor: ends.anchor.place(buffer),
            head: ends.head.place(buffer),
            goal: ends.goal,
        }))
    }
}
