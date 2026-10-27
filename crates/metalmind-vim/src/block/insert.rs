//! `I`, `A` y `c` sobre un bloque: se escribe en la primera línea y, al
//! salir de Insert con `<Esc>`, lo escrito se repite en las demás, como
//! `op_insert`, `block_insert` y `op_change` de vim.

use std::ops::Range;

use metalmind_buffer::{Buffer, ByteOffset, Edit};

use super::Block;
use super::prep::{Piece, Purpose, prep};
use crate::columns::Columns;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Insertion {
    /// `I`: antes del bloque; las líneas que no llegan a él no cambian.
    Insert,
    /// `A`: tras el bloque; las líneas cortas se rellenan hasta su borde,
    /// salvo tras `$`.
    Append,
    /// `c`: donde estaba el bloque, ya borrado.
    Change,
}

/// Lo que se mide de la primera línea al entrar a Insert, para saber al
/// salir qué se escribió.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Pending {
    insertion: Insertion,
    block: Block,
    /// El `bd` de vim: lo que el bloque abarcaba en la primera línea.
    piece: Piece,
    /// Bytes de la primera línea tras donde se escribe.
    pre_textlen: usize,
    /// El byte de la esquina del bloque en la primera línea, donde vuelve
    /// el cursor.
    corner: usize,
}

/// Dónde empieza Insert y los espacios que `A` agrega ahí antes si la
/// primera línea no llega al borde del bloque.
pub(crate) struct Start {
    pub pending: Pending,
    pub at: ByteOffset,
    pub padding: Option<Edit>,
}

/// Lo que se repite en las demás líneas al salir de Insert, y dónde queda
/// el cursor si cambia.
pub(crate) struct Finish {
    pub edits: Vec<Edit>,
    pub cursor: Option<ByteOffset>,
}

impl Pending {
    /// `I` o `A`, antes de entrar a Insert.
    pub fn insert(buffer: &Buffer, block: Block, append: bool, columns: Columns) -> Start {
        let (insertion, purpose) = if append {
            (Insertion::Append, Purpose::Append)
        } else {
            (Insertion::Insert, Purpose::Insert)
        };
        let line_start = buffer.line_start(block.first);
        let text = buffer.line_text(block.first);
        let mut piece = prep(&text, &block, purpose, columns);
        let mut pre_textlen = text.len() - piece.textcol;
        let corner = columns.byte_at(&text, block.start, true);
        let mut at = corner;
        let mut padding = None;
        if append {
            pre_textlen -= piece.textlen;
            let after = piece.textcol + piece.textlen;
            if corner < after {
                at = after.min(text.len());
            }
            if piece.is_short && !block.to_line_end {
                let spaces = " ".repeat(piece.endspaces);
                padding = Some(Edit::insert(line_start + at, spaces));
                piece.textlen += piece.endspaces;
            }
        }
        let pending = Self {
            insertion,
            block,
            piece,
            pre_textlen,
            corner,
        };
        Start {
            pending,
            at: line_start + at,
            padding,
        }
    }

    /// `c`, con el bloque ya borrado y el cursor donde lo dejó el borrado.
    /// Como vim, si el borrado lo llevó a la izquierda de la esquina (la
    /// línea terminaba en el bloque), escribe tras el último carácter.
    pub fn change(
        buffer: &Buffer,
        block: Block,
        cursor: ByteOffset,
        corner: usize,
        columns: Columns,
    ) -> Start {
        let line_start = buffer.line_start(block.first);
        let text = buffer.line_text(block.first);
        let mut at = cursor.get() - line_start.get();
        let next = columns.cells(&text).find(|cell| cell.end > at);
        if let Some(cell) = next.filter(|_| corner > at) {
            at = cell.end;
        }
        let piece = Piece {
            textcol: at,
            ..Piece::default()
        };
        let pending = Self {
            insertion: Insertion::Change,
            block,
            piece,
            pre_textlen: text.len(),
            corner,
        };
        Start {
            pending,
            at: line_start + at,
            padding: None,
        }
    }

    pub fn insertion(&self) -> Insertion {
        self.insertion
    }

    pub fn first_line(&self) -> usize {
        self.block.first
    }

    /// Lo escrito hasta ahora en la primera línea, como lo mide
    /// [`Pending::finish`]; vacío al principio de la línea si se borró de
    /// más.
    pub fn written(&self, buffer: &Buffer, columns: Columns) -> Range<ByteOffset> {
        let text = buffer.line_text(self.block.first);
        let line_start = buffer.line_start(self.block.first);
        match self.written_in(&text, columns) {
            Some((start, len)) => line_start + start..line_start + start + len,
            None => line_start..line_start,
        }
    }

    /// Dónde empieza lo escrito en `text`, la primera línea, y cuántos
    /// bytes ocupa; `None` si se borró de más.
    fn written_in(&self, text: &str, columns: Columns) -> Option<(usize, usize)> {
        match self.insertion {
            Insertion::Change => {
                let len = text.len().checked_sub(self.pre_textlen)?;
                Some((self.piece.textcol, len))
            }
            Insertion::Insert | Insertion::Append => self.inserted(text, columns),
        }
    }

    /// Repite en las demás líneas lo escrito en la primera; `None` si no se
    /// escribió nada o se borró de más.
    pub fn finish(&self, buffer: &Buffer, columns: Columns) -> Option<Finish> {
        if self.insertion == Insertion::Change && self.block.first == self.block.last {
            return None;
        }
        let text = buffer.line_text(self.block.first);
        let line_start = buffer.line_start(self.block.first);
        let (start, inserted) = self.written_in(&text, columns)?;
        let inserted = text
            .get(start..start + inserted)
            .filter(|s| !s.is_empty())?;
        let edits = match self.insertion {
            Insertion::Change => self.change_lines(buffer, inserted, columns),
            Insertion::Insert | Insertion::Append => self.insert_lines(buffer, inserted, columns),
        };
        let cursor = (self.insertion != Insertion::Change).then(|| line_start + self.corner);
        Some(Finish { edits, cursor })
    }

    /// Dónde empieza lo escrito con `I` o `A` en la primera línea y cuántos
    /// bytes ocupa, midiendo el bloque otra vez como `op_insert`.
    fn inserted(&self, text: &str, columns: Columns) -> Option<(usize, usize)> {
        let append = self.insertion == Insertion::Append;
        let purpose = if append {
            Purpose::Append
        } else {
            Purpose::Insert
        };
        let mut piece = self.piece;
        let mut pre_textlen = Some(self.pre_textlen);
        let again = prep(text, &self.block, purpose, columns);
        if !self.block.to_line_end || again.textlen < piece.textlen {
            let mut textlen = again.textlen;
            if append {
                pre_textlen = (self.pre_textlen + again.textlen).checked_sub(piece.textlen);
                if again.endspaces > 0 {
                    textlen = textlen.saturating_sub(1);
                }
            }
            piece.textcol = again.textcol;
            piece.textlen = textlen;
        }
        let written_after = if append { piece.textlen } else { 0 };
        let start = (piece.textcol + written_after).min(text.len());
        let len = (text.len() - start).checked_sub(pre_textlen?)?;
        Some((start, len))
    }

    /// `block_insert` de vim.
    fn insert_lines(&self, buffer: &Buffer, inserted: &str, columns: Columns) -> Vec<Edit> {
        let block = &self.block;
        let append = self.insertion == Insertion::Append;
        let purpose = if append {
            Purpose::Append
        } else {
            Purpose::Insert
        };
        let mut edits = Vec::new();
        for line in block.first + 1..=block.last {
            let text = buffer.line_text(line);
            let piece = prep(&text, block, purpose, columns);
            if piece.is_short && !append {
                continue;
            }
            let (tab_width, spaces, mut offset) = if !append {
                (piece.start_char_vcols, piece.startspaces, piece.textcol)
            } else if !piece.is_short {
                let spaces = if piece.endspaces > 0 {
                    piece.end_char_vcols - piece.endspaces
                } else {
                    0
                };
                let end = piece.textcol + piece.textlen;
                (piece.end_char_vcols, spaces, end - usize::from(spaces > 0))
            } else if block.to_line_end {
                (piece.end_char_vcols, 0, piece.textcol + piece.textlen)
            } else {
                let spaces = block.end - piece.end_vcol + 1;
                (piece.end_char_vcols, spaces, piece.textcol + piece.textlen)
            };
            if spaces > 0 {
                while !text.is_char_boundary(offset) {
                    offset -= 1;
                }
            }
            let mut middle = " ".repeat(spaces);
            middle.push_str(inserted);
            let mut removed = 0;
            let splits_tab = text
                .get(offset..)
                .is_some_and(|rest| rest.starts_with('\t'));
            if spaces > 0 && !piece.is_short && splits_tab {
                middle.push_str(&" ".repeat(tab_width - spaces));
                removed = 1;
            }
            let start = buffer.line_start(line) + offset;
            edits.push(Edit::replace(start..start + removed, middle));
        }
        edits
    }

    /// El final de `op_change`: lo escrito va donde empezaba el bloque en
    /// cada línea que llega a él.
    fn change_lines(&self, buffer: &Buffer, inserted: &str, columns: Columns) -> Vec<Edit> {
        let block = &self.block;
        let mut edits = Vec::new();
        for line in block.first + 1..=block.last {
            let text = buffer.line_text(line);
            let piece = prep(&text, block, Purpose::Delete, columns);
            if !piece.is_short {
                let at = buffer.line_start(line) + piece.textcol;
                edits.push(Edit::insert(at, inserted));
            }
        }
        edits
    }
}
