//! `y`, `d`, `~ u U` y `r` sobre un bloque, línea por línea.

use nota_buffer::{Buffer, Edit};
use unicode_width::UnicodeWidthChar;

use super::Block;
use super::plan::Plan;
use super::prep::{Purpose, prep};
use crate::columns::Columns;
use crate::register::{Register, Shape};
use crate::transform::Transform;

/// Lo que un operador hace con el texto del bloque, salvo copiarlo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BlockEdit {
    Delete,
    Transform(Transform),
}

/// Lo que copia `y`, como `op_yank` de vim: lo que el borde parte, como un
/// tab, va como espacios, y una línea que no llega al bloque, como una de
/// espacios.
pub(crate) fn yank(buffer: &Buffer, block: &Block, columns: Columns) -> Register {
    let lines: Vec<String> = block
        .lines()
        .map(|line| {
            let text = buffer.line_text(line);
            prep(&text, block, Purpose::Copy, columns).copied(&text)
        })
        .collect();
    let width = block.end - block.start;
    let width = if block.to_line_end {
        width.saturating_sub(1)
    } else {
        width
    };
    Register {
        text: lines.join("\n"),
        shape: Shape::Block { width },
    }
}

pub(crate) fn edit(buffer: &Buffer, block: &Block, how: BlockEdit, columns: Columns) -> Plan {
    match how {
        BlockEdit::Delete => delete(buffer, block, columns),
        BlockEdit::Transform(Transform::Replace(c)) => replace(buffer, block, c, columns),
        BlockEdit::Transform(transform) => change_case(buffer, block, transform, columns),
    }
}

/// Lo partido se rellena con espacios para que lo de después no se mueva
/// (`op_delete` de vim). El cursor va donde empezaba el bloque en la primera
/// línea.
pub(crate) fn delete(buffer: &Buffer, block: &Block, columns: Columns) -> Plan {
    let mut plan = Plan {
        edits: Vec::new(),
        at: block.top_left(buffer, columns),
        shift: 0,
    };
    for line in block.lines() {
        let text = buffer.line_text(line);
        let piece = prep(&text, block, Purpose::Delete, columns);
        if piece.textlen == 0 {
            continue;
        }
        let start = buffer.line_start(line) + piece.textcol;
        if line == block.first {
            plan.at = start;
            plan.shift = piece.startspaces;
        }
        let spaces = " ".repeat(piece.startspaces + piece.endspaces);
        plan.edits
            .push(Edit::replace(start..start + piece.textlen, spaces));
    }
    plan
}

/// `~`, `u` y `U` sobre los caracteres que el bloque toca, aunque sea en
/// parte (`op_tilde` de vim).
fn change_case(buffer: &Buffer, block: &Block, transform: Transform, columns: Columns) -> Plan {
    let mut edits = Vec::new();
    for line in block.lines() {
        let text = buffer.line_text(line);
        let piece = prep(&text, block, Purpose::Copy, columns);
        let old = piece.text(&text);
        let new = transform.text(old);
        if new != old {
            let start = buffer.line_start(line) + piece.textcol;
            edits.push(Edit::replace(start..start + piece.textlen, new));
        }
    }
    Plan {
        edits,
        at: block.top_left(buffer, columns),
        shift: 0,
    }
}

/// `r{c}`, como `op_replace` de vim: cada columna del bloque pasa a ser `c`,
/// o media si `c` es ancho; lo partido queda como espacios y las líneas
/// cortas no crecen.
fn replace(buffer: &Buffer, block: &Block, c: char, columns: Columns) -> Plan {
    let mut edits = Vec::new();
    for line in block.lines() {
        let text = buffer.line_text(line);
        let piece = prep(&text, block, Purpose::Replace, columns);
        if piece.textlen == 0 {
            continue;
        }
        let mut count = if piece.is_short {
            piece.end_vcol - block.start
        } else {
            block.end - block.start + 1
        };
        let mut endspaces = piece.endspaces;
        if c.width() == Some(2) {
            if count % 2 == 1 && !piece.is_short {
                endspaces += 1;
            }
            count /= 2;
        }
        let mut new = " ".repeat(piece.startspaces);
        new.extend(std::iter::repeat_n(c, count));
        if !piece.is_short {
            new.push_str(&" ".repeat(endspaces));
        }
        let start = buffer.line_start(line) + piece.textcol;
        edits.push(Edit::replace(start..start + piece.textlen, new));
    }
    Plan {
        edits,
        at: block.top_left(buffer, columns),
        shift: 0,
    }
}
