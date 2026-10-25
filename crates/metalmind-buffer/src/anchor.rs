//! Anclas de línea: posiciones que siguen a su línea cuando se insertan o
//! borran líneas antes, como las marks de vim (`mark_adjust`).

use std::ops::RangeInclusive;

use ropey::Rope;

use crate::LineEdit;
use crate::spot::Spot;

/// Una posición que sigue a su línea. La crea [`crate::Buffer::create_anchor`]
/// y solo vale en ese buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LineAnchor(usize);

/// Línea y bytes desde su inicio.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Point {
    pub line: usize,
    pub column: usize,
}

impl Point {
    #[must_use]
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
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

    /// `false` si el ancla no es de este buffer.
    pub fn clear(&mut self, anchor: LineAnchor) -> bool {
        self.0.get_mut(anchor.0).map(|slot| *slot = None).is_some()
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
/// Viene en cada [`Splice`](crate::Splice) para mover posiciones guardadas
/// por línea fuera del buffer, como la jumplist de vim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineChange {
    start: Point,
    end: Point,
    /// Borra hasta el final de una línea posterior a la primera.
    reaches_later_line_end: bool,
    /// Borra todo el texto: todas las líneas desaparecen.
    clears_text: bool,
    inserted_lines: usize,
    /// Columna donde termina lo insertado, en su última línea.
    tail: usize,
    /// Mete líneas enteras antes de la línea de `start`, como `O` o `P`.
    inserts_lines_before: bool,
}

impl LineChange {
    /// Reemplazar `start..end` de `rope` por `text`.
    pub(crate) fn between(rope: &Rope, start: &Spot, end: &Spot, text: &str) -> Self {
        let removed = end.byte - start.byte;
        let reaches_later_line_end = end.point.line > start.point.line && {
            let next = end.point.line + 1;
            let line_end = if next < rope.len_lines() {
                rope.line_to_byte(next) - 1
            } else {
                rope.len_bytes()
            };
            end.byte == line_end
        };
        let inserted_lines = text.matches('\n').count();
        let tail = match text.rfind('\n') {
            Some(newline) => text.len() - newline - 1,
            None => start.point.column + text.len(),
        };
        Self {
            start: start.point,
            end: end.point,
            reaches_later_line_end,
            clears_text: removed > 0 && removed == rope.len_bytes(),
            inserted_lines,
            tail,
            inserts_lines_before: removed == 0 && start.point.column == 0 && text.ends_with('\n'),
        }
    }

    /// Como [`Self::between`], con bytes.
    #[cfg(test)]
    pub(crate) fn new(rope: &Rope, start: usize, removed: usize, text: &str) -> Self {
        let spot = |byte| Spot::of(rope, byte).unwrap_or_else(|| panic!("{byte} fuera del texto"));
        Self::between(rope, &spot(start), &spot(start + removed), text)
    }

    /// Las líneas de antes que toca la edición; las de después se corren
    /// [`Self::line_shift`].
    pub(crate) fn lines(&self) -> RangeInclusive<usize> {
        self.start.line..=self.end.line
    }

    /// Lo mismo, línea a línea.
    pub(crate) fn line_edit(&self) -> LineEdit {
        LineEdit {
            first: self.start.line,
            removed: self.end.line - self.start.line + 1,
            inserted: self.inserted_lines + 1,
        }
    }

    pub(crate) fn line_shift(&self) -> isize {
        let removed = self.end.line - self.start.line;
        // Un texto que cabe en memoria no tiene más de `isize::MAX` líneas.
        self.inserted_lines.cast_signed() - removed.cast_signed()
    }

    /// Dónde queda `line` tras la edición; `None` si se borró.
    pub(crate) fn adjust_line(&self, line: usize) -> Option<usize> {
        self.adjust(Point { line, column: 0 })
            .map(|point| point.line)
    }

    /// Dónde queda `point` tras la edición, como una mark de vim; `None` si
    /// su línea se borró. En la línea editada la columna no cambia; lo que
    /// queda de una línea que se une a otra sí se corre con su texto.
    #[must_use]
    pub fn adjust(&self, point: Point) -> Option<Point> {
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

    /// Como [`Self::adjust`], pero si su línea se borra queda en la primera
    /// línea de lo borrado, como las entradas de la jumplist de vim
    /// (`one_adjust_nodel`).
    #[must_use]
    pub fn keep(&self, point: Point) -> Point {
        self.adjust(point).unwrap_or(Point {
            line: self.start.line,
            ..point
        })
    }

    /// Una línea desaparece si se borra con su salto de línea: las de en
    /// medio, la primera si se borra desde su inicio hasta el inicio de otra
    /// (`dd`) y, si no, la última si se borra hasta su final (`dd` en la
    /// última línea, o un borrado que sigue desde una línea anterior). Si
    /// las dos de los bordes están vacías, el rango no dice cuál se borró:
    /// se toma como un `dd` de la primera.
    fn vanishes(&self, line: usize) -> bool {
        let (first, last) = (self.start.line, self.end.line);
        if self.clears_text {
            return true;
        }
        if first == last {
            return false;
        }
        let whole_lines = self.start.column == 0 && self.end.column == 0;
        if line == first {
            whole_lines
        } else if line == last {
            !whole_lines && self.reaches_later_line_end
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
    }

    #[test]
    fn deleting_a_line_keeps_the_empty_one_below() {
        let text = "x\ny\n\nz";
        assert_eq!(moved(text, 2, 2, "", point(2, 0)), Some(point(1, 0)));
        assert_eq!(moved("x\n\n\ny", 2, 1, "", point(2, 0)), Some(point(1, 0)));
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
    fn keep_leaves_a_deleted_line_at_the_start_of_the_deletion() {
        let keep = |start, removed, at| {
            LineChange::new(&Rope::from_str(TEXT), start, removed, "").keep(at)
        };
        assert_eq!(keep(3, 3, point(1, 1)), point(1, 1));
        assert_eq!(keep(0, 6, point(1, 1)), point(0, 1));
        assert_eq!(keep(0, 6, point(2, 1)), point(0, 1));
        assert_eq!(keep(0, 8, point(2, 1)), point(0, 1));
    }

    #[test]
    fn columns_are_bytes() {
        assert_eq!(moved("añ\nb", 0, 0, "x\n", point(0, 3)), Some(point(1, 3)));
        assert_eq!(moved("añ\nb", 3, 1, "", point(1, 0)), Some(point(0, 3)));
    }
}
