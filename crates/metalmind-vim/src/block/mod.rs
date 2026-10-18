//! Visual por bloque (`ctrl-v`): las columnas de pantalla que abarca un
//! bloque y lo que cada operador hace en cada una de sus líneas, portado de
//! `ops.c` y `register.c` de nvim.

mod edit;
mod insert;
mod plan;
mod prep;
mod put;
mod shift;

use std::ops::{Range, RangeInclusive};

use metalmind_buffer::{Buffer, ByteOffset, Goal, Selection};

use crate::Viewport;
use crate::columns::Columns;
use prep::{Purpose, prep};

pub(crate) use edit::{BlockEdit, delete, edit, yank};
pub(crate) use insert::{Insertion, Pending};
pub(crate) use plan::{Plan, apply, without_overlaps};
pub(crate) use put::{put, put_in_each_line};
pub(crate) use shift::shift;

/// Las líneas `first..=last` entre las columnas de pantalla `start` y `end`,
/// las dos incluidas: el `oparg_T` de vim tras `get_op_vcol`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Block {
    pub first: usize,
    pub last: usize,
    pub start: usize,
    pub end: usize,
    /// Tras `$`: cada línea llega hasta su final.
    pub to_line_end: bool,
}

/// El tamaño de un bloque, para `.` y `[count]ctrl-v`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BlockSize {
    pub lines: usize,
    /// Columnas de ancho; `None` tras `$`.
    pub width: Option<usize>,
}

impl Block {
    /// El bloque entre los dos extremos de una selección: abarca los dos
    /// caracteres enteros, aunque sean un tab o uno ancho.
    pub fn of(buffer: &Buffer, selection: &Selection, columns: Columns) -> Self {
        let (top, bottom) = (selection.start(), selection.end());
        let (top_start, top_end) = columns.span_of(buffer, top);
        let (bottom_start, bottom_end) = columns.span_of(buffer, bottom);
        let first = buffer.line_of(top);
        let last = buffer.line_of(bottom);
        let to_line_end = selection.goal == Goal::LineEnd;
        let end = if to_line_end {
            widest(buffer, first..=last, columns)
        } else {
            top_end.max(bottom_end)
        };
        Self {
            first,
            last,
            start: top_start.min(bottom_start),
            end,
            to_line_end,
        }
    }

    /// El bloque de `size` que empieza en `head`, como lo repite `.`.
    pub fn sized(buffer: &Buffer, head: ByteOffset, size: BlockSize, columns: Columns) -> Self {
        let first = buffer.line_of(head);
        let last = first
            .saturating_add(size.lines.saturating_sub(1))
            .min(buffer.last_line());
        let (start, _) = columns.span_of(buffer, head);
        let end = match size.width {
            Some(width) => start + width.saturating_sub(1),
            None => widest(buffer, first..=last, columns),
        };
        Self {
            first,
            last,
            start,
            end,
            to_line_end: size.width.is_none(),
        }
    }

    pub fn size(&self) -> BlockSize {
        BlockSize {
            lines: self.last - self.first + 1,
            width: (!self.to_line_end).then(|| self.end - self.start + 1),
        }
    }

    pub fn lines(&self) -> RangeInclusive<usize> {
        self.first..=self.last
    }

    /// La esquina de arriba a la izquierda, donde vim deja el cursor antes
    /// de operar; en una línea corta, su fin de línea.
    pub fn top_left(&self, buffer: &Buffer, columns: Columns) -> ByteOffset {
        columns.offset_at(buffer, self.first, self.start, true)
    }
}

/// Lo que abarca una selección de Visual por bloque, para pintarla (ver el
/// contrato en el README del crate).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VisualBlock {
    block: Block,
    columns: Columns,
}

impl VisualBlock {
    /// El bloque de `selection`, con las columnas medidas con el `tabstop`
    /// de `viewport`.
    #[must_use]
    pub fn new(buffer: &Buffer, selection: &Selection, viewport: &Viewport) -> Self {
        let columns = viewport.columns();
        Self {
            block: Block::of(buffer, selection, columns),
            columns,
        }
    }

    #[must_use]
    pub fn lines(&self) -> RangeInclusive<usize> {
        self.block.lines()
    }

    /// Primera y última columna de pantalla, las dos incluidas. Tras `$`
    /// cada línea llega hasta su final, y la última es la del fin de la
    /// línea más ancha.
    #[must_use]
    pub fn columns(&self) -> RangeInclusive<usize> {
        self.block.start..=self.block.end
    }

    #[must_use]
    pub fn to_line_end(&self) -> bool {
        self.block.to_line_end
    }

    /// Los bytes de `line` dentro del bloque, con enteros los caracteres que
    /// sus bordes parten, como un tab; vacío al final de una línea que no
    /// llega al bloque.
    #[must_use]
    pub fn bytes_on(&self, buffer: &Buffer, line: usize) -> Range<ByteOffset> {
        let text = buffer.line_text(line);
        let piece = prep(&text, &self.block, Purpose::Delete, self.columns);
        let start = buffer.line_start(line) + piece.textcol;
        start..start + piece.textlen
    }
}

/// La columna del fin de la línea más ancha, como mide vim un bloque tras `$`.
fn widest(buffer: &Buffer, lines: RangeInclusive<usize>, columns: Columns) -> usize {
    lines
        .map(|line| columns.line_width(&buffer.line_text(line)))
        .max()
        .unwrap_or(0)
}
