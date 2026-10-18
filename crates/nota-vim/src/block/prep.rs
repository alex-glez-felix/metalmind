//! Lo que un bloque abarca en una línea: un port de `block_prep` de nvim.

use super::Block;
use crate::columns::{Cell, Columns};

/// Para qué se mide la línea; cambia cómo se cuentan los caracteres que el
/// borde del bloque parte por la mitad, como un tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Purpose {
    /// Copiar o cambiar mayúsculas: lo partido se copia como espacios.
    Copy,
    /// Borrar o cambiar: lo partido se borra entero y se rellena con espacios.
    Delete,
    Insert,
    Append,
    Replace,
}

impl Purpose {
    fn deletes(self) -> bool {
        self != Self::Copy
    }
}

/// El `block_def` de vim: el texto del bloque en una línea, con los espacios
/// que reemplazan a lo partido. Bytes dentro de la línea; columnas de
/// pantalla.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Piece {
    pub textcol: usize,
    pub textlen: usize,
    pub startspaces: usize,
    pub endspaces: usize,
    /// La columna donde empieza el texto, y donde termina.
    pub start_vcol: usize,
    pub end_vcol: usize,
    /// La línea termina antes del borde derecho del bloque, o del izquierdo.
    pub is_short: bool,
    /// Un solo carácter cubre el bloque entero.
    pub is_one_char: bool,
    /// Lo que ocupa el carácter en el borde izquierdo y en el derecho.
    pub start_char_vcols: usize,
    pub end_char_vcols: usize,
}

impl Piece {
    pub fn text<'a>(&self, line: &'a str) -> &'a str {
        line.get(self.textcol..self.textcol + self.textlen)
            .unwrap_or_default()
    }

    /// Lo que copia `y`: el texto con lo partido como espacios.
    pub fn copied(&self, line: &str) -> String {
        format!(
            "{}{}{}",
            " ".repeat(self.startspaces),
            self.text(line),
            " ".repeat(self.endspaces)
        )
    }
}

pub(crate) fn prep(line: &str, block: &Block, purpose: Purpose, columns: Columns) -> Piece {
    let deletes = purpose.deletes();
    let cells: Vec<Cell> = columns.cells(line).collect();
    let start_of = |index: usize| cells.get(index).map_or(line.len(), |cell| cell.start);
    let mut piece = Piece::default();
    let (mut index, mut incr, mut vcol) = (0, 0, 0);
    while vcol < block.start && index < cells.len() {
        incr = cells[index].width;
        vcol += incr;
        index += 1;
    }
    let prev_pstart = start_of(index.saturating_sub(1));
    let pstart = start_of(index);
    piece.start_vcol = vcol;
    piece.start_char_vcols = incr;
    if vcol < block.start {
        piece.end_vcol = vcol;
        piece.is_short = true;
        if !deletes || purpose == Purpose::Append {
            piece.endspaces = block.end - block.start + 1;
        }
        piece.textcol = pstart;
        return piece;
    }
    piece.startspaces = vcol - block.start;
    if deletes && piece.startspaces > 0 {
        piece.startspaces = incr - piece.startspaces;
    }
    piece.end_vcol = vcol;
    let pend = if vcol > block.end {
        piece.is_one_char = true;
        one_char(&mut piece, block, purpose);
        pstart
    } else {
        let (pend, last_width) = to_right_edge(&cells, index, pstart, block, purpose, &mut piece);
        incr = last_width;
        pend.unwrap_or(line.len())
    };
    piece.end_char_vcols = incr;
    let pstart = if deletes && piece.startspaces > 0 {
        prev_pstart
    } else {
        pstart
    };
    piece.textlen = pend - pstart;
    piece.textcol = pstart;
    piece
}

/// Sigue desde el carácter `index` hasta pasar el borde derecho del bloque.
/// Devuelve dónde termina el texto (`None` si en el fin de línea) y el ancho
/// del último carácter.
fn to_right_edge(
    cells: &[Cell],
    mut index: usize,
    pstart: usize,
    block: &Block,
    purpose: Purpose,
    piece: &mut Piece,
) -> (Option<usize>, usize) {
    let mut vcol = piece.end_vcol;
    let mut incr = piece.start_char_vcols;
    let mut prev_pend = pstart;
    while vcol <= block.end && index < cells.len() {
        prev_pend = cells[index].start;
        incr = cells[index].width;
        vcol += incr;
        index += 1;
    }
    piece.end_vcol = vcol;
    let mut pend = cells.get(index).map(|cell| cell.start);
    let deletes = purpose.deletes();
    let short_counts = !deletes || matches!(purpose, Purpose::Append | Purpose::Replace);
    if vcol <= block.end && short_counts {
        piece.is_short = true;
        if purpose == Purpose::Append {
            piece.endspaces = block.end - vcol + 1;
        }
    } else if vcol > block.end {
        piece.endspaces = vcol - block.end - 1;
        if !deletes && piece.endspaces > 0 {
            piece.endspaces = incr - piece.endspaces;
            if pend != Some(pstart) {
                pend = Some(prev_pend);
            }
        }
    }
    (pend, incr)
}

/// Un carácter que pasa los dos bordes, como un tab más ancho que el bloque.
fn one_char(piece: &mut Piece, block: &Block, purpose: Purpose) {
    let width = block.end - block.start + 1;
    match purpose {
        Purpose::Insert => piece.endspaces = piece.start_char_vcols - piece.startspaces,
        Purpose::Append => {
            piece.startspaces += width;
            piece.endspaces = piece.start_char_vcols - piece.startspaces;
        }
        Purpose::Copy => piece.startspaces = width,
        Purpose::Delete | Purpose::Replace => {
            piece.startspaces = piece.start_char_vcols - (piece.start_vcol - block.start);
            piece.endspaces = piece.end_vcol - block.end - 1;
        }
    }
}
