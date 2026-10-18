//! `>` y `<` sobre un bloque, como `shift_block` de nvim: corren lo que
//! empieza en el borde izquierdo de cada línea, no la sangría.

use std::ops::Range;

use metalmind_buffer::{Buffer, Edit};

use super::Block;
use super::plan::Plan;
use super::prep::{Piece, Purpose, is_white, prep};
use crate::columns::Columns;
use crate::indent::{Indent, Shift};

/// Las ediciones de cada línea que llega al bloque; las vacías y las que
/// terminan antes de su borde izquierdo no cambian. `at` es el inicio de la
/// primera línea: el cursor vuelve a su byte de antes, como en vim.
pub(crate) fn shift(
    buffer: &Buffer,
    block: &Block,
    shift: Shift,
    levels: usize,
    columns: Columns,
    indent: Indent,
) -> Plan {
    let purpose = match shift {
        Shift::Right => Purpose::ShiftRight,
        Shift::Left => Purpose::ShiftLeft,
    };
    let mut edits = Vec::new();
    for line in block.lines() {
        let text = buffer.line_text(line);
        let piece = prep(&text, block, purpose, columns);
        if text.is_empty() || piece.is_short {
            continue;
        }
        let (range, new) = match shift {
            Shift::Right => right(&text, &piece, indent.levels_width(levels), columns, indent),
            Shift::Left => left(&text, &piece, block, indent.levels_width(levels), columns),
        };
        if text.get(range.clone()) != Some(new.as_str()) {
            let start = buffer.line_start(line);
            edits.push(Edit::replace(start + range.start..start + range.end, new));
        }
    }
    Plan {
        edits,
        at: buffer.line_start(block.first),
        shift: 0,
    }
}

/// Junta los blancos que tocan el borde izquierdo con los `width` que se
/// agregan y los vuelve a escribir desde donde empezaban.
fn right(
    line: &str,
    piece: &Piece,
    width: usize,
    columns: Columns,
    indent: Indent,
) -> (Range<usize>, String) {
    let mut total = width + piece.pre_whitesp;
    let mut ws_vcol = piece.start_vcol - piece.pre_whitesp;
    let mut textstart = piece.textcol;
    let mut splits = piece.startspaces != 0;
    if splits {
        // Un tab partido ya cuenta entre los blancos; uno ancho, no.
        if char_len(line, textstart) == 1 {
            textstart += 1;
        } else {
            ws_vcol = 0;
            splits = false;
        }
    }
    let from = textstart;
    for cell in columns.cells(line).skip_while(|cell| cell.start < from) {
        if !is_white(line, cell) {
            break;
        }
        total += cell.width;
        textstart = cell.end;
    }
    let first_white = (piece.textcol + usize::from(splits)).saturating_sub(piece.pre_whitesp_c);
    let new = indent.whitespace(ws_vcol, ws_vcol + total, columns);
    (first_white..textstart, new)
}

/// Quita hasta `width` columnas de los blancos que siguen al borde
/// izquierdo; un tab que queda a medias se cambia por espacios.
fn left(
    line: &str,
    piece: &Piece,
    block: &Block,
    width: usize,
    columns: Columns,
) -> (Range<usize>, String) {
    let splits = piece.startspaces != 0;
    let mut non_white = piece.textcol;
    if splits {
        non_white += char_len(line, non_white);
    }
    let mut non_white_col = piece.start_vcol;
    let from = non_white;
    for cell in columns.cells(line).skip_while(|cell| cell.start < from) {
        if !is_white(line, cell) {
            break;
        }
        non_white_col += cell.width;
        non_white = cell.end;
    }
    let block_space_width = non_white_col.saturating_sub(block.start);
    let destination = non_white_col - block_space_width.min(width);
    let mut kept_end = piece.textcol;
    let mut kept_width = piece.start_vcol;
    if splits {
        kept_width -= piece.start_char_vcols;
    }
    let from = kept_end;
    for cell in columns.cells(line).skip_while(|cell| cell.start < from) {
        if kept_width >= destination || kept_width + cell.width > destination {
            break;
        }
        kept_width += cell.width;
        kept_end = cell.end;
    }
    (kept_end..non_white, " ".repeat(destination - kept_width))
}

/// Bytes del carácter en `byte` con los que se combinan con él, como
/// `utfc_ptr2len`; 0 en el fin de línea.
fn char_len(line: &str, byte: usize) -> usize {
    let rest = line.get(byte..).unwrap_or_default();
    Columns::default()
        .cells(rest)
        .next()
        .map_or(0, |cell| cell.end)
}
