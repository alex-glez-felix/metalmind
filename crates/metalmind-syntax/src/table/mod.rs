//! Las tablas GFM de una nota: cuál contiene una línea, en qué celda cae un
//! byte y cómo queda alineada.
//!
//! Una tabla es una fila de encabezado seguida de la separadora (`|---|`)
//! con tantas celdas como ella, y las filas que siguen mientras tengan algún
//! `|`, estén en la misma cita y no empiecen otro bloque. Celdas y cursor se
//! miden en bytes; el ancho de una columna, en columnas de pantalla.

mod row;

#[cfg(test)]
mod tests;

use std::ops::Range;

use unicode_width::UnicodeWidthStr;

use crate::style::Lines;

pub use row::Align;
use row::Row;

/// El ancho mínimo de una columna, el de `---` y `:-:`, como prettier.
const MIN_WIDTH: usize = 3;

/// Una tabla, con el texto de sus filas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    first_line: usize,
    lines: Vec<String>,
    rows: Vec<Row>,
    aligns: Vec<Align>,
}

/// Una celda: su fila (el encabezado es la 0 y la separadora la 1) y su
/// columna.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellPos {
    pub row: usize,
    pub column: usize,
}

/// Hacia dónde salta `tab` (`Next`) o `shift-tab` (`Previous`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Next,
    Previous,
}

/// Un punto dentro de una celda: cuántos bytes de su contenido, sin los
/// blancos de los lados, quedan antes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellSpot {
    pub cell: CellPos,
    pub offset: usize,
}

impl Table {
    /// La tabla que contiene la línea `line`, si la hay.
    #[must_use]
    pub fn around(lines: &impl Lines, line: usize) -> Option<Self> {
        if line >= lines.line_count() {
            return None;
        }
        let quotes = Row::parse(&lines.line(line))?.quotes;
        let is_row =
            |index: usize| Row::parse(&lines.line(index)).is_some_and(|row| row.quotes == quotes);
        let first = (0..line)
            .rev()
            .take_while(|&i| is_row(i))
            .last()
            .unwrap_or(line);
        let end = (line + 1..lines.line_count())
            .find(|&i| !is_row(i))
            .unwrap_or(lines.line_count());
        let mut block: Vec<String> = (first..end).map(|i| lines.line(i).into_owned()).collect();
        let header = header_index(&block)?;
        if first + header > line {
            return None;
        }
        Some(Self::from_lines(first + header, block.split_off(header)))
    }

    fn from_lines(first_line: usize, lines: Vec<String>) -> Self {
        let rows: Vec<Row> = lines.iter().filter_map(|line| Row::parse(line)).collect();
        let aligns = rows
            .get(1)
            .zip(lines.get(1))
            .and_then(|(row, line)| row.alignments(line))
            .unwrap_or_default();
        Self {
            first_line,
            lines,
            rows,
            aligns,
        }
    }

    /// Las líneas de la nota que ocupa.
    #[must_use]
    pub fn lines(&self) -> Range<usize> {
        self.first_line..self.first_line + self.rows.len()
    }

    /// La celda del byte `at` de la línea `line`. Un `|` es de la celda a su
    /// izquierda; los blancos de una celda, del lado de su contenido más
    /// cercano.
    #[must_use]
    pub fn spot(&self, line: usize, at: usize) -> CellSpot {
        let row = line
            .saturating_sub(self.first_line)
            .min(self.rows.len() - 1);
        let cells = &self.rows[row].cells;
        let column = cells
            .iter()
            .rposition(|cell| cell.raw.start <= at)
            .unwrap_or(0);
        let offset = cells.get(column).map_or(0, |cell| {
            at.clamp(cell.content.start, cell.content.end) - cell.content.start
        });
        CellSpot {
            cell: CellPos { row, column },
            offset,
        }
    }

    /// Cada fila con sus `|` alineados: las celdas rellenas al ancho de su
    /// columna según la separadora, las que faltan en una fila corta vacías
    /// y las de más conservadas. La sangría y la cita son las del
    /// encabezado.
    #[must_use]
    pub fn formatted(&self) -> Formatted {
        let widths = self.widths();
        let mut formatted = Formatted {
            lines: Vec::with_capacity(self.rows.len() + 1),
            cells: Vec::with_capacity(self.rows.len() + 1),
        };
        for (index, row) in self.rows.iter().enumerate() {
            let contents = (0..self.columns(index)).map(|column| {
                if index == 1 {
                    let align = self.aligns.get(column).copied().unwrap_or(Align::None);
                    separator(align, widths[column])
                } else {
                    row.cells
                        .get(column)
                        .map_or("", |cell| &self.lines[index][cell.content.clone()])
                        .to_owned()
                }
            });
            self.push_row(&mut formatted, &widths, contents);
        }
        formatted
    }

    /// Como [`Table::formatted`], con una fila vacía más al final, de
    /// tantas celdas como el encabezado.
    #[must_use]
    pub fn formatted_with_new_row(&self) -> Formatted {
        let mut formatted = self.formatted();
        let empty = std::iter::repeat_n(String::new(), self.aligns.len());
        self.push_row(&mut formatted, &self.widths(), empty);
        formatted
    }

    /// La celda a la que lleva `step` desde `cell`, en orden de lectura y
    /// sin pasar por la separadora. Antes de la primera se queda en ella;
    /// tras la última, `None`: ahí va una fila nueva.
    #[must_use]
    pub fn step(&self, cell: CellPos, step: Step) -> Option<CellPos> {
        let CellPos { row, column } = cell;
        let in_row = row != 1;
        let at = |row, column| Some(CellPos { row, column });
        match step {
            Step::Next if in_row && column + 1 < self.columns(row) => at(row, column + 1),
            Step::Next => {
                let next = if row == 0 { 2 } else { row + 1 };
                (next < self.rows.len()).then_some(CellPos {
                    row: next,
                    column: 0,
                })
            }
            Step::Previous if in_row && column > 0 => at(row, column - 1),
            Step::Previous if row == 0 => at(0, 0),
            Step::Previous => {
                let previous = if row == 2 { 0 } else { row - 1 };
                at(previous, self.columns(previous).saturating_sub(1))
            }
        }
    }

    /// Cuántas celdas tiene la fila `row` una vez alineada.
    fn columns(&self, row: usize) -> usize {
        if row == 1 {
            self.aligns.len()
        } else {
            self.rows[row].cells.len().max(self.aligns.len())
        }
    }

    /// Agrega a `formatted` una fila con `contents` rellenas a `widths`.
    fn push_row(
        &self,
        formatted: &mut Formatted,
        widths: &[usize],
        contents: impl Iterator<Item = String>,
    ) {
        let mut line = self.lines[0][self.rows[0].prefix.clone()].to_owned();
        let mut cells = Vec::new();
        line.push('|');
        for (column, (content, &width)) in contents.zip(widths).enumerate() {
            let align = self.aligns.get(column).copied().unwrap_or(Align::None);
            let (before, after) = padding(align, width.saturating_sub(content.width()));
            line.push(' ');
            line.push_str(&" ".repeat(before));
            cells.push(line.len()..line.len() + content.len());
            line.push_str(&content);
            line.push_str(&" ".repeat(after));
            line.push_str(" |");
        }
        formatted.lines.push(line);
        formatted.cells.push(cells);
    }

    /// El ancho de cada columna: el de su celda más ancha, sin contar la
    /// separadora, y nunca menos que un separador.
    fn widths(&self) -> Vec<usize> {
        let columns = self
            .rows
            .iter()
            .map(|row| row.cells.len())
            .max()
            .unwrap_or(0);
        let mut widths = vec![0; columns];
        for (width, _) in widths.iter_mut().zip(&self.aligns) {
            *width = MIN_WIDTH;
        }
        for (index, row) in self.rows.iter().enumerate().filter(|&(i, _)| i != 1) {
            for (width, cell) in widths.iter_mut().zip(&row.cells) {
                *width = (*width).max(self.lines[index][cell.content.clone()].width());
            }
        }
        widths
    }
}

/// Una tabla ya alineada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Formatted {
    lines: Vec<String>,
    /// El contenido de cada celda, en bytes sobre su línea.
    cells: Vec<Vec<Range<usize>>>,
}

impl Formatted {
    /// Cada fila, en el orden de la tabla y sin salto de línea; con
    /// [`Table::formatted_with_new_row`], una más que la tabla.
    #[must_use]
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Dónde queda `spot` en el texto alineado: la fila y el byte en ella,
    /// en la misma celda y a la misma altura de su contenido.
    #[must_use]
    pub fn place(&self, spot: CellSpot) -> (usize, usize) {
        let row = spot.cell.row.min(self.cells.len() - 1);
        let cells = &self.cells[row];
        let at = cells
            .get(spot.cell.column)
            .or(cells.last())
            .map_or(0, |content| (content.start + spot.offset).min(content.end));
        (row, at)
    }
}

/// Dónde empieza la tabla en un bloque de filas: la primera fila seguida
/// de una separadora con tantas celdas como ella.
fn header_index(block: &[String]) -> Option<usize> {
    let rows: Vec<Row> = block.iter().filter_map(|line| Row::parse(line)).collect();
    rows.windows(2)
        .zip(block.iter().skip(1))
        .position(|(pair, line)| {
            pair[0].cells.len() == pair[1].cells.len() && pair[1].alignments(line).is_some()
        })
}

fn separator(align: Align, width: usize) -> String {
    let (left, right) = match align {
        Align::None => (false, false),
        Align::Left => (true, false),
        Align::Center => (true, true),
        Align::Right => (false, true),
    };
    let colons = usize::from(left) + usize::from(right);
    let mut text = String::with_capacity(width);
    if left {
        text.push(':');
    }
    text.push_str(&"-".repeat(width - colons));
    if right {
        text.push(':');
    }
    text
}

/// Los blancos antes y después del contenido de una celda.
fn padding(align: Align, spare: usize) -> (usize, usize) {
    match align {
        Align::None | Align::Left => (0, spare),
        Align::Right => (spare, 0),
        Align::Center => (spare / 2, spare - spare / 2),
    }
}
