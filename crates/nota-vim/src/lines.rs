use nota_buffer::{Buffer, ByteOffset, Goal, Selection};

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

/// Si `offset` está antes o sobre el primer no blanco de su línea.
pub(crate) fn in_indent(buffer: &Buffer, offset: ByteOffset) -> bool {
    offset <= first_non_blank(buffer, buffer.line_of(offset))
}

/// Columna que `j`/`k` intentan conservar, ya resuelta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Want {
    Column(usize),
    LineEnd,
}

impl Want {
    pub fn of(buffer: &Buffer, selection: &Selection) -> Self {
        match selection.goal {
            Goal::None => Self::Column(buffer.column_of(selection.head)),
            Goal::Column(column) => Self::Column(column),
            Goal::LineEnd => Self::LineEnd,
        }
    }

    /// Posición en Normal sobre `line` que mejor respeta esta columna.
    pub fn offset_on(self, buffer: &Buffer, line: usize) -> ByteOffset {
        match self {
            Self::Column(column) => clamp_normal(buffer, buffer.offset_at_column(line, column)),
            Self::LineEnd => last_char(buffer, line),
        }
    }

    /// Como [`Self::offset_on`], pero en Visual: puede quedar sobre el fin
    /// de línea, que así entra en la selección (`coladvance` de vim con
    /// `selection=inclusive`).
    pub fn visual_offset_on(self, buffer: &Buffer, line: usize) -> ByteOffset {
        match self {
            Self::Column(column) => buffer.offset_at_column(line, column),
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
