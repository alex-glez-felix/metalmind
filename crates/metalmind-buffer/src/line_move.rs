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
}

impl LineMove {
    /// `None` si quedan donde están: debajo de la de antes o de la última.
    pub fn new(lines: &RangeInclusive<usize>, below: usize) -> Option<Self> {
        let (first, last) = (*lines.start(), *lines.end());
        (below != first && below != last + 1).then_some(Self { first, last, below })
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
        let line_move = LineMove::new(&lines, below).unwrap();
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
        assert_eq!(LineMove::new(&(1..=2), 1), None);
        assert_eq!(LineMove::new(&(1..=2), 3), None);
    }

    #[test]
    fn the_column_stays() {
        let line_move = LineMove::new(&(2..=2), 0).unwrap();
        assert_eq!(line_move.point(Point::new(2, 5)), Point::new(0, 5));
    }
}
