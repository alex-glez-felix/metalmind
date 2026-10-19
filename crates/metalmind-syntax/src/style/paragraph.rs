//! Un párrafo leído una vez: el texto desde una línea hasta antes de la
//! primera en blanco o que abre una cerca, que es hasta donde mira el
//! código inline, y lo que el scanner indexó de él. Tras el cierre de un
//! comentario o de un bloque `$$` el estilo sigue en el mismo párrafo; si
//! cada tramo lo volviera a leer, un párrafo con muchos bloques costaría
//! cuadrático.

use std::ops::Range;

use super::Lines;
use crate::code::{self, opening_fence};
use crate::scanner::Lookahead;

pub(crate) struct Paragraph {
    /// La línea de la nota donde empieza.
    first: usize,
    /// Si sigue otra línea, termina con su salto, como en la nota.
    pub text: String,
    /// Cada línea en `text`, sin el salto.
    pub lines: Vec<Range<usize>>,
    pub look: Lookahead,
}

impl Paragraph {
    /// El párrafo desde la línea `first`, que ya se leyó: `line`.
    pub(crate) fn read(source: &impl Lines, first: usize, line: &str) -> Self {
        let mut text = line.to_owned();
        let mut lines = Vec::new();
        lines.push(0..line.len());
        for index in first + 1..source.line_count() {
            text.push('\n');
            let next = source.line(index);
            if code::is_blank(&next) || opening_fence(&next).is_some() {
                break;
            }
            let start = text.len();
            text.push_str(&next);
            lines.push(start..text.len());
        }
        Self {
            first,
            text,
            lines,
            look: Lookahead::default(),
        }
    }

    /// El índice en [`Paragraph::lines`] de la línea `line` de la nota, si
    /// es de este párrafo.
    pub(crate) fn index_of(&self, line: usize) -> Option<usize> {
        line.checked_sub(self.first)
            .filter(|&index| index < self.lines.len())
    }
}
