//! Una línea de tabla partida en celdas. Un `|` separa celdas salvo
//! escapado (`\|`) o dentro de código, una fórmula o un wikilink: partirlo
//! ahí cambiaría lo que dice la celda. Los blancos que se quitan de los
//! lados son espacios y tabs, como en GFM: un espacio duro es contenido.

use std::ops::Range;

use crate::anchor::heading_at;
use crate::code::{self, CodeSpans};
use crate::link::wikilink_at;
use crate::math::InlineMath;
use crate::style::{BlockKind, block_line, quote_prefix_len};

const BLANKS: [char; 2] = [' ', '\t'];

/// Una celda, en bytes sobre su línea.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Cell {
    /// Entre sus dos `|`, o entre el borde de la fila y su `|`.
    pub raw: Range<usize>,
    /// Sin los blancos de los lados.
    pub content: Range<usize>,
}

/// Una línea con al menos un `|` que separa celdas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Row {
    /// La sangría y los `>` de una cita.
    pub prefix: Range<usize>,
    /// Cuántas citas la contienen: una fila de otra cita es otro bloque.
    pub quotes: usize,
    pub cells: Vec<Cell>,
}

impl Row {
    /// `None` si la línea no tiene ningún `|` que separe celdas o empieza
    /// otro bloque: un heading, un ítem de lista, una regla o una cerca.
    pub(super) fn parse(line: &str) -> Option<Self> {
        let block = block_line(line);
        let quote = quote_prefix_len(line);
        let starts_block = block.kind != BlockKind::Paragraph
            || heading_at(line, quote).is_some()
            || code::fence(line).is_some();
        if starts_block {
            return None;
        }
        let body = line[quote..].trim_start_matches(BLANKS);
        let start = line.len() - body.len();
        let end = start + body.trim_end_matches([' ', '\t', '\r']).len();
        let pipes = pipes(line, start..end);
        let (&first, &last) = (pipes.first()?, pipes.last()?);
        let leading = first == start;
        let trailing = last + 1 == end && !(leading && pipes.len() == 1);
        let mut bounds = Vec::with_capacity(pipes.len() + 2);
        if !leading {
            bounds.push(start);
        }
        bounds.extend(pipes.iter().map(|&pipe| pipe + 1));
        let mut ends: Vec<usize> = pipes;
        if !trailing {
            ends.push(end);
        }
        if leading {
            ends.remove(0);
        }
        let cells = bounds
            .into_iter()
            .zip(ends)
            .map(|(from, to)| Cell {
                raw: from..to,
                content: trimmed(line, from..to),
            })
            .collect();
        Some(Self {
            prefix: 0..start,
            quotes: block.quotes.len(),
            cells,
        })
    }

    /// Los alineados de la fila separadora (`|:--|--:|`), si lo es.
    pub(super) fn alignments(&self, line: &str) -> Option<Vec<Align>> {
        self.cells
            .iter()
            .map(|cell| Align::parse(&line[cell.content.clone()]))
            .collect()
    }
}

/// Cómo alinea su columna la fila separadora.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    /// `---`
    None,
    /// `:--`
    Left,
    /// `:-:`
    Center,
    /// `--:`
    Right,
}

impl Align {
    fn parse(cell: &str) -> Option<Self> {
        let left = cell.starts_with(':');
        let right = cell.len() > 1 && cell.ends_with(':');
        let dashes = &cell[usize::from(left)..cell.len() - usize::from(right)];
        if dashes.is_empty() || dashes.bytes().any(|b| b != b'-') {
            return None;
        }
        Some(match (left, right) {
            (false, false) => Self::None,
            (true, false) => Self::Left,
            (true, true) => Self::Center,
            (false, true) => Self::Right,
        })
    }
}

/// Los `|` de `line[span]` que separan celdas.
fn pipes(line: &str, span: Range<usize>) -> Vec<usize> {
    let row = &line[..span.end];
    let bytes = row.as_bytes();
    let mut code = CodeSpans::default();
    let mut math = InlineMath::default();
    let mut found = Vec::new();
    let mut at = span.start;
    while at < bytes.len() {
        at = match bytes[at] {
            b'|' => {
                found.push(at);
                at + 1
            }
            b'\\' => at + 2,
            b'`' => code.end(row, at),
            b'$' => math.at(row, at).map_or(at + 1, |formula| formula.span.end),
            b'[' if bytes.get(at + 1) == Some(&b'[') => {
                wikilink_at(row, at, at).map_or(at + 2, |link| link.span.end)
            }
            _ => at + 1,
        };
    }
    found
}

fn trimmed(line: &str, range: Range<usize>) -> Range<usize> {
    let text = &line[range.clone()];
    let start = range.start + (text.len() - text.trim_start_matches(BLANKS).len());
    let end = range.start + text.trim_end_matches(BLANKS).len();
    start..end.max(start)
}
