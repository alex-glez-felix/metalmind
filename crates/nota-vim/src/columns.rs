//! Columnas de pantalla, como las cuenta vim sin `list` ni `linebreak`
//! (`win_charsize`): un tab llega hasta el siguiente múltiplo de `tabstop`,
//! un carácter ancho ocupa dos y uno de control se ve como `^X` o `<80>`.
//! Las usan `j`/`k` para conservar la columna y los bloques de `ctrl-v`.

use std::iter::Peekable;
use std::str::CharIndices;

use nota_buffer::{Buffer, ByteOffset};
use unicode_width::UnicodeWidthChar;

/// El `tabstop` de nvim por default.
pub(crate) const DEFAULT_TABSTOP: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Columns {
    tabstop: usize,
}

impl Default for Columns {
    fn default() -> Self {
        Self::new(DEFAULT_TABSTOP)
    }
}

/// Un carácter de una línea, con los que se combinan con él, y dónde cae en
/// pantalla. Los offsets son bytes dentro de la línea.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Cell {
    pub start: usize,
    pub end: usize,
    pub vcol: usize,
    pub width: usize,
    pub is_tab: bool,
}

impl Columns {
    pub fn new(tabstop: usize) -> Self {
        Self {
            tabstop: tabstop.max(1),
        }
    }

    pub fn tabstop(self) -> usize {
        self.tabstop
    }

    pub fn cells(self, line: &str) -> Cells<'_> {
        Cells {
            chars: line.char_indices().peekable(),
            vcol: 0,
            columns: self,
        }
    }

    /// Primera y última columna del carácter en `offset` del buffer.
    pub fn span_of(self, buffer: &Buffer, offset: ByteOffset) -> (usize, usize) {
        let place = self.place_of(buffer, offset);
        (place.start, place.end)
    }

    fn place_of(self, buffer: &Buffer, offset: ByteOffset) -> Place {
        let line = buffer.line_of(offset);
        let byte = offset.get() - buffer.line_start(line).get();
        self.place_at(&buffer.line_text(line), byte)
    }

    /// El carácter de `line` que ocupa la columna `vcol`, como `coladvance`;
    /// ver [`Columns::byte_at`].
    pub fn offset_at(
        self,
        buffer: &Buffer,
        line: usize,
        vcol: usize,
        past_end: bool,
    ) -> ByteOffset {
        let byte = self.byte_at(&buffer.line_text(line), vcol, past_end);
        buffer.line_start(line) + byte
    }

    /// Dónde se ve el cursor sobre `offset` (el `w_virtcol` de vim): al
    /// inicio del carácter, salvo un tab con `tab_end`, que lo muestra al
    /// final, como en Normal.
    pub fn cursor_at(self, buffer: &Buffer, offset: ByteOffset, tab_end: bool) -> usize {
        let place = self.place_of(buffer, offset);
        if place.on_tab && tab_end {
            place.end
        } else {
            place.start
        }
    }

    /// Ancho de la línea entera: la columna de su fin de línea.
    pub fn line_width(self, line: &str) -> usize {
        self.cells(line)
            .last()
            .map_or(0, |cell| cell.vcol + cell.width)
    }

    /// Ancho de `text` con cada carácter como si empezara en la columna 0,
    /// como mide vim una línea de un registro por bloque al pegarla.
    pub fn loose_width(self, text: &str) -> usize {
        self.cells(text)
            .map(|cell| {
                if cell.is_tab {
                    self.tabstop
                } else {
                    cell.width
                }
            })
            .sum()
    }

    /// Primera y última columna del carácter que empieza en `byte`, como
    /// `getvvcol`; en el fin de línea, una sola columna tras el texto.
    pub fn span_at(self, line: &str, byte: usize) -> (usize, usize) {
        let place = self.place_at(line, byte);
        (place.start, place.end)
    }

    fn place_at(self, line: &str, byte: usize) -> Place {
        let mut vcol = 0;
        for cell in self.cells(line) {
            if cell.end > byte {
                return Place {
                    start: cell.vcol,
                    end: cell.vcol + cell.width - 1,
                    on_tab: cell.is_tab,
                };
            }
            vcol = cell.vcol + cell.width;
        }
        Place {
            start: vcol,
            end: vcol,
            on_tab: false,
        }
    }

    /// El byte del carácter que ocupa la columna `vcol`, como `coladvance`.
    /// Si la línea no llega, el fin de línea o, sin `past_end`, su último
    /// carácter.
    pub fn byte_at(self, line: &str, vcol: usize, past_end: bool) -> usize {
        let mut last = 0;
        for cell in self.cells(line) {
            if cell.vcol + cell.width > vcol {
                return cell.start;
            }
            last = cell.start;
        }
        if past_end { line.len() } else { last }
    }

    fn width(self, c: char, vcol: usize) -> usize {
        match c {
            '\t' => self.tabstop - vcol % self.tabstop,
            c if c.is_control() && c.is_ascii() => 2,
            c if c.is_control() => 4,
            c => c.width().unwrap_or(1).max(1),
        }
    }
}

/// Las columnas del carácter en un byte, y si es un tab: medirlo en la
/// misma pasada evita volver al rope a preguntar qué carácter es.
#[derive(Clone, Copy)]
struct Place {
    start: usize,
    end: usize,
    on_tab: bool,
}

pub(crate) struct Cells<'a> {
    chars: Peekable<CharIndices<'a>>,
    vcol: usize,
    columns: Columns,
}

impl Iterator for Cells<'_> {
    type Item = Cell;

    fn next(&mut self) -> Option<Cell> {
        let (start, c) = self.chars.next()?;
        let mut end = start + c.len_utf8();
        while let Some(&(at, next)) = self.chars.peek() {
            if !is_composing(next) {
                break;
            }
            end = at + next.len_utf8();
            self.chars.next();
        }
        let width = self.columns.width(c, self.vcol);
        let cell = Cell {
            start,
            end,
            vcol: self.vcol,
            width,
            is_tab: c == '\t',
        };
        self.vcol += width;
        Some(cell)
    }
}

/// Va pegado al carácter de antes, como los que vim llama `composing`.
fn is_composing(c: char) -> bool {
    !c.is_control() && c.width() == Some(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_reach_the_next_stop_and_wide_chars_take_two() {
        let columns = Columns::new(4);
        let widths: Vec<_> = columns
            .cells("a\tb日\u{1}é")
            .map(|cell| (cell.vcol, cell.width))
            .collect();
        assert_eq!(widths, [(0, 1), (1, 3), (4, 1), (5, 2), (7, 2), (9, 1)]);
        assert_eq!(columns.line_width("a\tb"), 5);
    }

    #[test]
    fn composing_chars_ride_with_their_base() {
        let cells: Vec<_> = Columns::default().cells("e\u{301}x").collect();
        assert_eq!(cells.len(), 2);
        assert_eq!((cells[0].end, cells[1].vcol), (3, 1));
    }

    #[test]
    fn a_column_lands_on_the_char_that_covers_it() {
        let columns = Columns::new(8);
        assert_eq!(columns.span_at("a\tb", 1), (1, 7));
        assert_eq!(columns.span_at("a\tb", 3), (9, 9));
        assert_eq!(columns.byte_at("a\tb", 5, false), 1);
        assert_eq!(columns.byte_at("a\tb", 8, false), 2);
        assert_eq!(columns.byte_at("a\tb", 20, false), 2);
        assert_eq!(columns.byte_at("a\tb", 20, true), 3);
        assert_eq!(columns.byte_at("", 3, false), 0);
    }
}
