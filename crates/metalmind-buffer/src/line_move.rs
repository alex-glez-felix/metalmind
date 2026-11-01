//! Líneas que cambian de lugar sin cambiar su texto, como con `:m` de vim.

use std::ops::RangeInclusive;

use crate::Point;

/// Las líneas `first..=last` pasan a quedar debajo de la línea `below`,
/// contada desde 1; 0 es arriba de todo. `below` nunca cae entre ellas ni
/// las deja donde estaban.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LineMove {
    first: usize,
    last: usize,
    below: usize,
    swap: Swap,
}

/// Lo mismo en bytes del texto de antes: los tramos `start..middle` y
/// `middle..end`, de líneas enteras, cambian de lugar. Si el segundo llega
/// a la última línea, `end` pasa un byte del final, como si tuviera salto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Swap {
    start: usize,
    middle: usize,
    end: usize,
}

impl LineMove {
    /// `None` si quedan donde están: debajo de la de antes o de la última.
    /// `line_start` da el byte donde empieza cada línea del texto de antes
    /// y, para la que sigue a la última, uno más que el largo del texto.
    pub fn new(
        lines: &RangeInclusive<usize>,
        below: usize,
        line_start: impl Fn(usize) -> usize,
    ) -> Option<Self> {
        let (first, last) = (*lines.start(), *lines.end());
        if below == first || below == last + 1 {
            return None;
        }
        let [start, middle, end] = if below < first {
            [below, first, last + 1]
        } else {
            [first, last + 1, below]
        }
        .map(line_start);
        Some(Self {
            first,
            last,
            below,
            swap: Swap { start, middle, end },
        })
    }

    /// Dónde queda el byte `offset` del texto de antes: con su línea, en la
    /// misma columna.
    pub fn offset(&self, offset: usize) -> usize {
        let Swap { start, middle, end } = self.swap;
        if (start..middle).contains(&offset) {
            offset + (end - middle)
        } else if (middle..end).contains(&offset) {
            offset - (middle - start)
        } else {
            offset
        }
    }

    /// Lo que sigue a su línea va con ella; la columna no cambia.
    pub fn point(&self, point: Point) -> Point {
        Point {
            line: self.line(point.line),
            ..point
        }
    }

    fn line(&self, line: usize) -> usize {
        let moved = self.last - self.first + 1;
        let lines = self.first..=self.last;
        if self.below < self.first {
            if lines.contains(&line) {
                line - self.first + self.below
            } else if (self.below..self.first).contains(&line) {
                line + moved
            } else {
                line
            }
        } else if lines.contains(&line) {
            line - self.first + self.below - moved
        } else if (self.last + 1..self.below).contains(&line) {
            line - moved
        } else {
            line
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dónde queda cada una de `count` líneas tras mover `lines` debajo de
    /// `below`.
    fn moved(count: usize, lines: RangeInclusive<usize>, below: usize) -> Vec<usize> {
        let line_move = LineMove::new(&lines, below, |line| line).unwrap();
        (0..count).map(|line| line_move.line(line)).collect()
    }

    #[test]
    fn moving_up_shifts_the_lines_in_between_down() {
        assert_eq!(moved(4, 1..=1, 0), [1, 0, 2, 3]);
        assert_eq!(moved(5, 3..=4, 1), [0, 3, 4, 1, 2]);
        assert_eq!(moved(5, 3..=4, 0), [2, 3, 4, 0, 1]);
    }

    #[test]
    fn moving_down_shifts_the_lines_in_between_up() {
        assert_eq!(moved(5, 0..=1, 5), [3, 4, 0, 1, 2]);
        assert_eq!(moved(5, 0..=0, 4), [3, 0, 1, 2, 4]);
        assert_eq!(moved(3, 0..=0, 3), [2, 0, 1]);
    }

    #[test]
    fn below_the_line_before_or_the_last_one_is_no_move() {
        assert_eq!(LineMove::new(&(1..=2), 1, |line| line), None);
        assert_eq!(LineMove::new(&(1..=2), 3, |line| line), None);
    }

    #[test]
    fn the_column_stays() {
        let line_move = LineMove::new(&(2..=2), 0, |line| line).unwrap();
        assert_eq!(line_move.point(Point::new(2, 5)), Point::new(0, 5));
    }

    /// "a\nbb\nc" sin salto final: las líneas empiezan en 0, 2 y 5, y la
    /// que seguiría, en 7.
    #[test]
    fn bytes_go_with_their_line() {
        let starts = [0, 2, 5, 7];
        let bytes = |lines: RangeInclusive<usize>, below| {
            let line_move = LineMove::new(&lines, below, |line| starts[line]).unwrap();
            (0..=6)
                .map(|offset| line_move.offset(offset))
                .collect::<Vec<_>>()
        };
        // "c\na\nbb"
        assert_eq!(bytes(2..=2, 0), [2, 3, 4, 5, 6, 0, 1]);
        // "bb\nc\na"
        assert_eq!(bytes(0..=0, 3), [5, 6, 0, 1, 2, 3, 4]);
        // "bb\na\nc"
        assert_eq!(bytes(1..=1, 0), [3, 4, 0, 1, 2, 5, 6]);
    }
}
