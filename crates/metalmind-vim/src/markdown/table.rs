//! Alinear la tabla GFM de cada cursor: `editor::format_table`, `tab` y
//! `shift-tab` entre celdas en Insert y lo que alinea al salir de Insert.

use std::ops::Range;

use metalmind_buffer::{Bias, Buffer, ByteOffset, Edit, EditMap, Selection, Selections};
use metalmind_syntax::{CellPos, CellSpot, Formatted, Step, Table};

use super::LazyStyles;
use crate::VimError;
use crate::doc::BufferLines;

/// Qué hace cada cursor con su tabla.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Move {
    /// Se queda en su celda.
    Stay,
    /// Salta a otra celda; tras la última agrega una fila. Fuera de una
    /// tabla escribe un tab.
    Cell(Step),
}

/// Alinea la tabla de cada cursor, en la transacción abierta, y dice dónde
/// quedan los cursores y si alguno estaba en una tabla.
pub(crate) fn align_tables(
    buffer: &mut Buffer,
    selections: &Selections,
    styles: &LazyStyles,
    motion: Move,
) -> Result<(Selections, bool), VimError> {
    let mut tables: Vec<TableFormat> = Vec::new();
    let targets = selections.map_to(|selection| {
        let table = TableFormat::at(buffer, selection.head, styles)?;
        let spot = table.spot(buffer, selection.head);
        let seen = tables.iter().position(|seen| seen.lines() == table.lines());
        let index = seen.unwrap_or_else(|| {
            tables.push(table);
            tables.len() - 1
        });
        Some((index, tables[index].moved(spot, motion)))
    });
    let mut edits: Vec<Edit> = tables
        .iter()
        .flat_map(|table| table.edits(buffer))
        .collect();
    let tab = matches!(motion, Move::Cell(_));
    if tab {
        edits.extend(
            selections
                .iter()
                .zip(targets.iter())
                .filter(|(_, target)| target.is_none())
                .map(|(selection, _)| Edit::insert(selection.head, "\t")),
        );
    }
    let map = buffer.apply(edits)?;
    let bias = if tab { Bias::After } else { Bias::Before };
    let mut targets = targets.iter();
    let after = selections.map(|selection| {
        Selection::cursor(match targets.next().copied().flatten() {
            Some((index, spot)) => tables[index].place(buffer, &map, spot),
            None => map.map(selection.head, bias),
        })
    });
    Ok((after, !tables.is_empty()))
}

/// Una tabla de la nota y cómo queda alineada.
struct TableFormat {
    table: Table,
    /// Dónde empieza, antes de alinear: una fila nueva en una tabla de
    /// arriba corre las de abajo.
    start: ByteOffset,
    formatted: Formatted,
    /// Si `formatted` trae una fila más que la tabla.
    grown: bool,
}

impl TableFormat {
    /// La tabla de la línea de `at`, si no está en un bloque de código ni
    /// en el frontmatter.
    fn at(buffer: &Buffer, at: ByteOffset, styles: &LazyStyles) -> Option<Self> {
        let line = buffer.line_of(at);
        let table = Table::around(&BufferLines(buffer), line)?;
        if styles.outline(buffer).is_literal(line) {
            return None;
        }
        let formatted = table.formatted();
        Some(Self {
            start: buffer.line_start(table.lines().start),
            table,
            formatted,
            grown: false,
        })
    }

    fn lines(&self) -> Range<usize> {
        self.table.lines()
    }

    /// Dónde queda `spot` tras `motion`; pasar de la última celda agrega
    /// una fila, una sola aunque lo pidan varios cursores.
    fn moved(&mut self, spot: CellSpot, motion: Move) -> CellSpot {
        let Move::Cell(step) = motion else {
            return spot;
        };
        let cell = self.table.step(spot.cell, step).unwrap_or_else(|| {
            if !self.grown {
                self.formatted = self.table.formatted_with_new_row();
                self.grown = true;
            }
            CellPos {
                row: self.formatted.lines().len() - 1,
                column: 0,
            }
        });
        CellSpot { cell, offset: 0 }
    }

    /// Una edición por fila que cambia y, si la hay, la fila nueva tras la
    /// última. El `\r` de un archivo CRLF se queda, y la fila nueva lo lleva.
    fn edits(&self, buffer: &Buffer) -> Vec<Edit> {
        let mut edits: Vec<Edit> = self
            .lines()
            .zip(self.formatted.lines())
            .filter_map(|(line, formatted)| {
                let text = buffer.line_text(line);
                let row = text.strip_suffix('\r').unwrap_or(&text);
                let start = buffer.line_start(line);
                (row != formatted).then(|| Edit::replace(start..start + row.len(), formatted))
            })
            .collect();
        if let Some(new_row) = self.formatted.lines().get(self.lines().len()) {
            let last = self.lines().end - 1;
            let text = buffer.line_text(last);
            let (row, newline) = match text.strip_suffix('\r') {
                Some(row) => (row, "\r\n"),
                None => (&*text, "\n"),
            };
            let at = buffer.line_start(last) + row.len();
            edits.push(Edit::insert(at, format!("{newline}{new_row}")));
        }
        edits
    }

    /// La celda donde está `at`, antes de alinear.
    fn spot(&self, buffer: &Buffer, at: ByteOffset) -> CellSpot {
        let line = buffer.line_of(at);
        self.table
            .spot(line, at.get() - buffer.line_start(line).get())
    }

    /// Dónde queda `spot` una vez aplicadas, con `map`, las
    /// [`TableFormat::edits`] de todas las tablas.
    fn place(&self, buffer: &Buffer, map: &EditMap, spot: CellSpot) -> ByteOffset {
        let (row, byte) = self.formatted.place(spot);
        let first = buffer.line_of(map.map(self.start, Bias::Before));
        buffer.line_start(first + row) + byte
    }
}
