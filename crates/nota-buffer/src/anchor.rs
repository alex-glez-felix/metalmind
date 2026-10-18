//! Anclas de línea: posiciones que siguen a su línea cuando se insertan o
//! borran líneas antes, como las marks de vim (`mark_adjust`).

use ropey::Rope;

/// Una posición que sigue a su línea. La crea [`crate::Buffer::create_anchor`]
/// y solo vale en ese buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LineAnchor(usize);

/// Línea y bytes desde su inicio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Point {
    pub line: usize,
    pub column: usize,
}

impl Point {
    pub fn of(rope: &Rope, byte: usize) -> Self {
        let line = rope.byte_to_line(byte);
        Self {
            line,
            column: byte - rope.line_to_byte(line),
        }
    }
}

/// Dónde está cada ancla; `None` si su línea se borró.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Anchors(Vec<Option<Point>>);

impl Anchors {
    pub fn insert(&mut self, point: Point) -> LineAnchor {
        self.0.push(Some(point));
        LineAnchor(self.0.len() - 1)
    }

    /// `false` si el ancla no es de este buffer.
    pub fn set(&mut self, anchor: LineAnchor, point: Point) -> bool {
        self.0
            .get_mut(anchor.0)
            .map(|slot| *slot = Some(point))
            .is_some()
    }

    pub fn get(&self, anchor: LineAnchor) -> Option<Point> {
        self.0.get(anchor.0).copied().flatten()
    }

    /// Como el undo de vim: vuelven a donde estaban en `saved` las anclas
    /// que ahí existían; las demás se quedan como están.
    pub fn restore(&mut self, saved: &Self) {
        for (slot, saved) in self.0.iter_mut().zip(&saved.0) {
            if saved.is_some() {
                *slot = *saved;
            }
        }
    }

    pub fn adjust(&mut self, change: &LineChange) {
        for slot in &mut self.0 {
            *slot = slot.and_then(|point| change.adjust(point));
        }
    }
}

/// Lo que una edición le hace a las líneas, medido sobre el texto de antes.
pub(crate) struct LineChange {
    start: Point,
    end: Point,
    end_line_len: usize,
    /// Borra todo el texto: todas las líneas desaparecen.
    clears_text: bool,
    inserted_lines: usize,
    /// Columna donde termina lo insertado, en su última línea.
    tail: usize,
    /// Mete líneas enteras antes de la línea de `start`, como `O` o `P`.
    inserts_lines_before: bool,
}

impl LineChange {
    pub fn new(rope: &Rope, start: usize, removed: usize, text: &str) -> Self {
        let end = start + removed;
        let line_len = |line| {
            let next = rope.line_to_byte(line + 1);
            let has_newline = line + 1 < rope.len_lines();
            next - usize::from(has_newline) - rope.line_to_byte(line)
        };
        let (start, end) = (Point::of(rope, start), Point::of(rope, end));
        let inserted_lines = text.matches('\n').count();
        let tail = match text.rfind('\n') {
            Some(newline) => text.len() - newline - 1,
            None => start.column + text.len(),
        };
        Self {
            start,
            end,
            end_line_len: line_len(end.line),
            clears_text: removed > 0 && removed == rope.len_bytes(),
            inserted_lines,
            tail,
            inserts_lines_before: removed == 0 && start.column == 0 && text.ends_with('\n'),
        }
    }

    /// Dónde queda `point` tras la edición; `None` si su línea se borró.
    /// En la línea editada la columna no cambia, como en vim; lo que queda
    /// de una línea que se une a otra sí se corre con su texto.
    fn adjust(&self, point: Point) -> Option<Point> {
        let (first, last) = (self.start.line, self.end.line);
        if point.line < first {
            return Some(point);
        }
        if point.line > last {
            let line = point.line - (last - first) + self.inserted_lines;
            return Some(Point { line, ..point });
        }
        if self.vanishes(point.line) {
            return None;
        }
        if first == last {
            let shift = if self.inserts_lines_before {
                self.inserted_lines
            } else {
                0
            };
            return Some(Point {
                line: point.line + shift,
                ..point
            });
        }
        if point.line == first {
            return Some(point);
        }
        let column = match point.column.checked_sub(self.end.column) {
            Some(past_end) => self.tail + past_end,
            None => self.tail,
        };
        Some(Point {
            line: first + self.inserted_lines,
            column,
        })
    }

    /// Una línea desaparece si se borra con su salto de línea: las de en
    /// medio, la primera si se borra desde su inicio hasta el inicio de otra
    /// (`dd`) y la última si se borra hasta su final (`dd` en la última
    /// línea, o un borrado que sigue desde una línea anterior).
    fn vanishes(&self, line: usize) -> bool {
        let (first, last) = (self.start.line, self.end.line);
        if self.clears_text {
            return true;
        }
        if first == last {
            return false;
        }
        if line == first {
            self.start.column == 0 && self.end.column == 0
        } else if line == last {
            self.end.column == self.end_line_len
        } else {
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(line: usize, column: usize) -> Point {
        Point { line, column }
    }

    /// Dónde queda `at` tras reemplazar `start..start + removed` por `text`.
    fn moved(text: &str, start: usize, removed: usize, inserted: &str, at: Point) -> Option<Point> {
        LineChange::new(&Rope::from_str(text), start, removed, inserted).adjust(at)
    }

    const TEXT: &str = "ab\ncd\nef";

    #[test]
    fn lines_above_shift_the_anchor() {
        assert_eq!(moved(TEXT, 0, 3, "", point(2, 1)), Some(point(1, 1)));
        assert_eq!(moved(TEXT, 2, 0, "\nxy", point(2, 1)), Some(point(3, 1)));
        assert_eq!(moved(TEXT, 3, 0, "xy\n", point(1, 1)), Some(point(2, 1)));
    }

    #[test]
    fn edits_below_or_on_the_same_line_keep_it() {
        assert_eq!(moved(TEXT, 6, 2, "", point(1, 1)), Some(point(1, 1)));
        assert_eq!(moved(TEXT, 3, 1, "xyz", point(1, 1)), Some(point(1, 1)));
        assert_eq!(moved(TEXT, 4, 0, "\n", point(1, 1)), Some(point(1, 1)));
    }

    #[test]
    fn deleting_its_line_drops_it() {
        assert_eq!(moved(TEXT, 3, 3, "", point(1, 1)), None);
        assert_eq!(moved(TEXT, 3, 5, "", point(2, 1)), None);
        assert_eq!(moved(TEXT, 3, 5, "", point(1, 1)), Some(point(1, 1)));
        assert_eq!(moved(TEXT, 5, 3, "", point(2, 0)), None);
        assert_eq!(moved(TEXT, 0, 8, "", point(0, 1)), None);
        assert_eq!(moved(TEXT, 1, 6, "", point(1, 0)), None);
    }

    #[test]
    fn a_joined_line_moves_with_its_text() {
        assert_eq!(moved(TEXT, 2, 1, "", point(1, 1)), Some(point(0, 3)));
        assert_eq!(moved(TEXT, 2, 1, " ", point(1, 1)), Some(point(0, 4)));
        assert_eq!(moved(TEXT, 1, 3, "", point(1, 0)), Some(point(0, 1)));
    }

    #[test]
    fn columns_are_bytes() {
        assert_eq!(moved("añ\nb", 0, 0, "x\n", point(0, 3)), Some(point(1, 3)));
        assert_eq!(moved("añ\nb", 3, 1, "", point(1, 0)), Some(point(0, 3)));
    }
}
