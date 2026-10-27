//! `editor::format_table`: alinear la tabla GFM del cursor.

use std::ops::Range;

use metalmind_buffer::{Buffer, ByteOffset, Edit};
use metalmind_syntax::{CellSpot, Formatted, Table};

use super::LazyStyles;
use crate::doc::BufferLines;

/// Una tabla de la nota y cómo queda alineada.
pub(crate) struct TableFormat {
    table: Table,
    formatted: Formatted,
}

impl TableFormat {
    /// La tabla de la línea de `at`, si no está en un bloque de código ni
    /// en el frontmatter.
    pub(crate) fn at(buffer: &Buffer, at: ByteOffset, styles: &LazyStyles) -> Option<Self> {
        let line = buffer.line_of(at);
        let table = Table::around(&BufferLines(buffer), line)?;
        if styles.outline(buffer).is_literal(line) {
            return None;
        }
        let formatted = table.formatted();
        Some(Self { table, formatted })
    }

    pub(crate) fn lines(&self) -> Range<usize> {
        self.table.lines()
    }

    /// Una edición por fila que cambia. El `\r` de un archivo CRLF se queda.
    pub(crate) fn edits(&self, buffer: &Buffer) -> Vec<Edit> {
        self.lines()
            .zip(self.formatted.lines())
            .filter_map(|(line, formatted)| {
                let text = buffer.line_text(line);
                let row = text.strip_suffix('\r').unwrap_or(&text);
                let start = buffer.line_start(line);
                (row != formatted).then(|| Edit::replace(start..start + row.len(), formatted))
            })
            .collect()
    }

    /// La celda donde está `at`, antes de alinear.
    pub(crate) fn spot(&self, buffer: &Buffer, at: ByteOffset) -> CellSpot {
        let line = buffer.line_of(at);
        self.table
            .spot(line, at.get() - buffer.line_start(line).get())
    }

    /// Dónde queda `spot` una vez aplicadas las [`TableFormat::edits`], que
    /// no cambian cuántas líneas hay.
    pub(crate) fn place(&self, buffer: &Buffer, spot: CellSpot) -> ByteOffset {
        let (row, byte) = self.formatted.place(spot);
        buffer.line_start(self.lines().start + row) + byte
    }
}
