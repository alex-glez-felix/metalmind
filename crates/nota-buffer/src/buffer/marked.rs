//! Líneas marcadas del buffer, para `:g` de vim.

use super::Buffer;
use crate::marked::MarkedLines;

impl Buffer {
    /// Marca `lines`, en orden creciente, y olvida las marcas de antes.
    /// Siguen a su línea con cada edición, como las de `:g` de vim, y se
    /// pierden si se borra.
    pub fn mark_lines(&mut self, lines: impl IntoIterator<Item = usize>) {
        self.marked = MarkedLines::new(lines);
    }

    /// La primera línea que sigue marcada, y le quita la marca.
    pub fn pop_marked_line(&mut self) -> Option<usize> {
        self.marked.pop_first()
    }

    pub fn clear_marked_lines(&mut self) {
        self.marked = MarkedLines::default();
    }
}
