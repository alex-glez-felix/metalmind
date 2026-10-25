//! Anclas de línea del buffer, para las marks de vim.

use super::Buffer;
use crate::anchor::Point;
use crate::{BufferError, ByteOffset, LineAnchor};

impl Buffer {
    /// Ancla de línea en `offset`: sigue a su línea cuando se insertan o
    /// borran líneas antes y la pierde si se borra su línea, como una mark
    /// de vim. El undo la devuelve a donde estaba antes del cambio deshecho.
    ///
    /// # Errors
    /// Si `offset` no cae en un límite de carácter del texto.
    pub fn create_anchor(&mut self, offset: ByteOffset) -> Result<LineAnchor, BufferError> {
        let point = self.point_at(offset)?;
        Ok(self.anchors.insert(point))
    }

    /// Lleva `anchor` a `offset`, aunque se hubiera perdido.
    ///
    /// # Errors
    /// Si `offset` no cae en un límite de carácter o el ancla es de otro
    /// buffer.
    pub fn move_anchor(
        &mut self,
        anchor: LineAnchor,
        offset: ByteOffset,
    ) -> Result<(), BufferError> {
        let point = self.point_at(offset)?;
        if self.anchors.set(anchor, point) {
            Ok(())
        } else {
            Err(BufferError::UnknownAnchor(anchor))
        }
    }

    /// Pierde `anchor` como si se hubiera borrado su línea, como `:delm`
    /// de vim: el undo de un cambio de cuando existía lo devuelve.
    ///
    /// # Errors
    /// Si el ancla es de otro buffer.
    pub fn clear_anchor(&mut self, anchor: LineAnchor) -> Result<(), BufferError> {
        if self.anchors.clear(anchor) {
            Ok(())
        } else {
            Err(BufferError::UnknownAnchor(anchor))
        }
    }

    /// Dónde está `anchor`; `None` si se borró su línea. Si la línea quedó
    /// más corta que la columna guardada, el final de la línea.
    #[must_use]
    pub fn anchor_offset(&self, anchor: LineAnchor) -> Option<ByteOffset> {
        Some(self.offset_at_point(self.anchors.get(anchor)?))
    }

    /// La línea de `offset` y los bytes desde su inicio.
    ///
    /// # Errors
    /// Si `offset` no cae en un límite de carácter del texto.
    pub fn point_at(&self, offset: ByteOffset) -> Result<Point, BufferError> {
        Ok(self.spots(&(offset..offset))?.0.point)
    }

    /// El offset de `point`, dentro del texto: una línea de más es la
    /// última y una columna de más, el fin de su línea; si cae a mitad de
    /// un carácter, su inicio.
    #[must_use]
    pub fn offset_at_point(&self, point: Point) -> ByteOffset {
        let line = point.line.min(self.last_line());
        let offset = (self.line_start(line) + point.column).min(self.line_end(line));
        let char_start = self.rope.char_to_byte(self.rope.byte_to_char(offset.get()));
        ByteOffset::new(char_start)
    }
}
