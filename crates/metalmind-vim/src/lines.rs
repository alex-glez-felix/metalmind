use metalmind_buffer::{Buffer, ByteOffset, Goal, Selection};

use crate::Mode;
use crate::columns::Columns;

pub(crate) fn is_blank(c: char) -> bool {
    c == ' ' || c == '\t'
}

/// Primer carácter no blanco, o el fin de línea si no hay ninguno.
pub(crate) fn first_non_blank(buffer: &Buffer, line: usize) -> ByteOffset {
    let end = buffer.line_end(line);
    let mut offset = buffer.line_start(line);
    while offset < end && buffer.char_at(offset).is_some_and(is_blank) {
        offset = buffer.next_boundary(offset);
    }
    offset
}

/// Último carácter de la línea, o su inicio si está vacía.
pub(crate) fn last_char(buffer: &Buffer, line: usize) -> ByteOffset {
    let start = buffer.line_start(line);
    let end = buffer.line_end(line);
    if end > start {
        buffer.prev_boundary(end)
    } else {
        start
    }
}

/// En Normal el cursor nunca queda sobre el fin de una línea con texto.
pub(crate) fn clamp_normal(buffer: &Buffer, offset: ByteOffset) -> ByteOffset {
    let offset = offset.min(buffer.len());
    let line = buffer.line_of(offset);
    if offset >= buffer.line_end(line) {
        last_char(buffer, line)
    } else {
        offset
    }
}

/// El byte `column` de `line`, sin pasar de su fin ni quedar a mitad de un
/// carácter, como `check_cursor_col` de vim tras cambiar de línea.
pub(crate) fn at_byte_column(buffer: &Buffer, line: usize, column: usize) -> ByteOffset {
    let mut at = (buffer.line_start(line) + column).min(buffer.line_end(line));
    while !buffer.is_char_boundary(at) {
        at = at.saturating_sub(1);
    }
    at
}

/// Si `offset` está antes o sobre el primer no blanco de su línea.
pub(crate) fn in_indent(buffer: &Buffer, offset: ByteOffset) -> bool {
    offset <= first_non_blank(buffer, buffer.line_of(offset))
}

/// Si el cursor sobre un tab se ve en su final, como en Normal; en Visual,
/// solo si va después del otro extremo, y en Insert nunca (`getvvcol` de
/// vim).
pub(crate) fn tab_end(mode: Mode, selection: &Selection) -> bool {
    match mode {
        Mode::Visual(_) => selection.head > selection.anchor,
        Mode::Insert => false,
        Mode::Normal | Mode::OperatorPending(_) | Mode::CommandLine => true,
    }
}

/// Columna de pantalla que `j`/`k` intentan conservar, ya resuelta: el
/// `curswant` de vim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Want {
    Column(usize),
    LineEnd,
}

impl Want {
    /// Sin columna guardada, la del cursor, que sobre un tab se ve en su
    /// final si `tab_end` (ver [`tab_end`]).
    pub fn of(buffer: &Buffer, selection: &Selection, tab_end: bool, columns: Columns) -> Self {
        match selection.goal {
            Goal::None => Self::Column(columns.cursor_at(buffer, selection.head, tab_end)),
            Goal::Column(column) => Self::Column(column),
            Goal::LineEnd => Self::LineEnd,
        }
    }

    /// Posición en Normal sobre `line` que mejor respeta esta columna: el
    /// carácter que la ocupa o, si la línea no llega, el último.
    pub fn offset_on(self, buffer: &Buffer, line: usize, columns: Columns) -> ByteOffset {
        match self {
            Self::Column(column) => columns.offset_at(buffer, line, column, false),
            Self::LineEnd => last_char(buffer, line),
        }
    }

    /// Como [`Self::offset_on`], pero en Visual: puede quedar sobre el fin
    /// de línea, que así entra en la selección (`coladvance` de vim con
    /// `selection=inclusive`).
    pub fn visual_offset_on(self, buffer: &Buffer, line: usize, columns: Columns) -> ByteOffset {
        match self {
            Self::Column(column) => columns.offset_at(buffer, line, column, true),
            Self::LineEnd => buffer.line_end(line),
        }
    }

    pub fn goal(self) -> Goal {
        match self {
            Self::Column(column) => Goal::Column(column),
            Self::LineEnd => Goal::LineEnd,
        }
    }
}
