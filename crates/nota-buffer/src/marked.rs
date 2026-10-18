//! Líneas marcadas, como las de `:g` en vim (`ml_setmarked`): siguen a su
//! línea con cada edición, también de undo, y se pierden si se borra.

use crate::anchor::LineChange;

/// Las marcas van en orden y ninguna edición lo cambia, así que las que
/// quedan después de una edición se corren todas lo mismo: ese corrimiento
/// se suma a un sufijo en un árbol de Fenwick y cada edición cuesta
/// O(log² n) más lo que toque en sus propias líneas.
#[derive(Debug, Default)]
pub(crate) struct MarkedLines {
    /// La línea de cada marca, sin los corrimientos de `shifts`.
    base: Vec<isize>,
    /// Si la marca sigue: se pierde al borrar su línea o al tomarla.
    alive: Vec<bool>,
    shifts: Fenwick,
    /// Las anteriores ya se tomaron o se perdieron.
    next: usize,
}

impl MarkedLines {
    /// `lines` en orden creciente.
    pub fn new(lines: impl IntoIterator<Item = usize>) -> Self {
        // Un texto que cabe en memoria no tiene más de `isize::MAX` líneas.
        let base: Vec<isize> = lines.into_iter().map(usize::cast_signed).collect();
        Self {
            alive: vec![true; base.len()],
            shifts: Fenwick::new(base.len()),
            base,
            next: 0,
        }
    }

    /// La primera marca que sigue, ya sin marcar.
    pub fn pop_first(&mut self) -> Option<usize> {
        while self.next < self.base.len() {
            let index = self.next;
            self.next += 1;
            if self.alive[index] {
                self.alive[index] = false;
                return Some(self.line(index).cast_unsigned());
            }
        }
        None
    }

    pub fn adjust(&mut self, change: &LineChange) {
        let lines = change.lines();
        let first = self.first_from(self.next, |line| line >= lines.start().cast_signed());
        let after = self.first_from(first, |line| line > lines.end().cast_signed());
        for index in first..after {
            let line = self.line(index).cast_unsigned();
            if let Some(line) = change.adjust_line(line) {
                self.set_line(index, line.cast_signed());
            } else {
                // Queda en la línea de la edición para no romper el orden.
                self.alive[index] = false;
                self.set_line(index, lines.start().cast_signed());
            }
        }
        if after < self.base.len() {
            self.shifts.add(after, change.line_shift());
        }
    }

    fn line(&self, index: usize) -> isize {
        self.base[index] + self.shifts.prefix_sum(index)
    }

    fn set_line(&mut self, index: usize, line: isize) {
        self.base[index] = line - self.shifts.prefix_sum(index);
    }

    /// El primer índice desde `from` cuya línea cumple `holds`, que una vez
    /// que se cumple se sigue cumpliendo.
    fn first_from(&self, from: usize, holds: impl Fn(isize) -> bool) -> usize {
        let (mut low, mut high) = (from, self.base.len());
        while low < high {
            let middle = low + (high - low) / 2;
            if holds(self.line(middle)) {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        low
    }
}

/// Sumas de prefijos que se actualizan en O(log n).
#[derive(Debug, Default)]
struct Fenwick(Vec<isize>);

impl Fenwick {
    fn new(len: usize) -> Self {
        Self(vec![0; len + 1])
    }

    /// Suma `delta` a la posición `index`, y con eso a todos los prefijos
    /// que la incluyen.
    fn add(&mut self, index: usize, delta: isize) {
        let mut at = index + 1;
        while at < self.0.len() {
            self.0[at] += delta;
            at += at.isolate_lowest_one();
        }
    }

    /// La suma de las posiciones `0..=index`.
    fn prefix_sum(&self, index: usize) -> isize {
        let mut at = index + 1;
        let mut sum = 0;
        while at > 0 {
            sum += self.0[at];
            at -= at.isolate_lowest_one();
        }
        sum
    }
}

#[cfg(test)]
mod tests {
    use ropey::Rope;

    use super::*;

    /// Reemplaza `start..start + removed` de `text` por `inserted` y corre
    /// las marcas.
    fn edit(marks: &mut MarkedLines, text: &str, start: usize, removed: usize, inserted: &str) {
        marks.adjust(&LineChange::new(
            &Rope::from_str(text),
            start,
            removed,
            inserted,
        ));
    }

    fn drain(mut marks: MarkedLines) -> Vec<usize> {
        std::iter::from_fn(|| marks.pop_first()).collect()
    }

    const TEXT: &str = "a\nb\nc\nd\ne\n";

    #[test]
    fn follow_their_lines_and_drop_deleted_ones() {
        let mut marks = MarkedLines::new([0, 2, 4]);
        edit(&mut marks, TEXT, 2, 2, "");
        assert_eq!(drain(marks), [0, 1, 3]);
        let mut marks = MarkedLines::new([0, 2, 4]);
        edit(&mut marks, TEXT, 4, 2, "");
        assert_eq!(drain(marks), [0, 3]);
        let mut marks = MarkedLines::new([0, 2, 4]);
        edit(&mut marks, TEXT, 0, 0, "x\ny\n");
        assert_eq!(drain(marks), [2, 4, 6]);
    }

    #[test]
    fn taken_marks_do_not_move_again() {
        let mut marks = MarkedLines::new([1, 3]);
        assert_eq!(marks.pop_first(), Some(1));
        edit(&mut marks, TEXT, 2, 2, "");
        assert_eq!(marks.pop_first(), Some(2));
        assert_eq!(marks.pop_first(), None);
    }

    /// Como `:g/./m0`: cada línea marcada sube al principio.
    #[test]
    fn many_edits_keep_every_mark_in_place() {
        let mut text = String::from(TEXT);
        let mut marks = MarkedLines::new(0..5);
        let mut moved = Vec::new();
        while let Some(line) = marks.pop_first() {
            let rope = Rope::from_str(&text);
            let start = rope.line_to_byte(line);
            let content = text[start..=start].to_owned();
            edit(&mut marks, &text, start, 2, "");
            text.replace_range(start..start + 2, "");
            edit(&mut marks, &text, 0, 0, &format!("{content}\n"));
            text.insert_str(0, &format!("{content}\n"));
            moved.push(line);
        }
        assert_eq!(moved, [0, 1, 2, 3, 4]);
    }
}
