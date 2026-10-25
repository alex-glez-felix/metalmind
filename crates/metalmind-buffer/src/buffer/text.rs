//! Lo que se lee del texto: líneas, caracteres y columnas.

use std::borrow::Cow;
use std::ops::Range;

use super::Buffer;
use crate::spot;
use crate::{BufferError, ByteOffset};

impl Buffer {
    #[must_use]
    pub fn len(&self) -> ByteOffset {
        ByteOffset::new(self.rope.len_bytes())
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rope.len_bytes() == 0
    }

    #[must_use]
    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    #[must_use]
    pub fn last_line(&self) -> usize {
        self.line_count() - 1
    }

    /// Línea que contiene `offset`; los offsets fuera del texto van a la última.
    #[must_use]
    pub fn line_of(&self, offset: ByteOffset) -> usize {
        self.rope.byte_to_line(offset.min(self.len()).get())
    }

    #[must_use]
    pub fn line_start(&self, line: usize) -> ByteOffset {
        ByteOffset::new(self.rope.line_to_byte(line.min(self.last_line())))
    }

    /// Offset del `\n` que cierra la línea, o el final del texto.
    #[must_use]
    pub fn line_end(&self, line: usize) -> ByteOffset {
        if line < self.last_line() {
            ByteOffset::new(self.rope.line_to_byte(line + 1) - 1)
        } else {
            self.len()
        }
    }

    #[must_use]
    pub fn is_char_boundary(&self, offset: ByteOffset) -> bool {
        spot::is_char_boundary(&self.rope, offset.get())
    }

    /// Carácter que empieza en `offset`.
    #[must_use]
    pub fn char_at(&self, offset: ByteOffset) -> Option<char> {
        let byte = offset.get();
        // Un offset en el borde de dos trozos cae en el de después.
        let (chunk, chunk_byte, ..) = self.rope.get_chunk_at_byte(byte)?;
        chunk.get(byte - chunk_byte..)?.chars().next()
    }

    /// Carácter que termina en `offset`.
    #[must_use]
    pub fn char_before(&self, offset: ByteOffset) -> Option<char> {
        let byte = offset.get();
        let (chunk, chunk_byte, ..) = self.rope.get_chunk_at_byte(byte)?;
        let before = chunk.get(..byte - chunk_byte)?;
        if !before.is_empty() || byte == 0 {
            return before.chars().next_back();
        }
        let (chunk, chunk_byte, ..) = self.rope.get_chunk_at_byte(byte - 1)?;
        chunk.get(..byte - chunk_byte)?.chars().next_back()
    }

    /// Inicio del carácter siguiente; el final del texto se queda donde está.
    #[must_use]
    pub fn next_boundary(&self, offset: ByteOffset) -> ByteOffset {
        self.char_at(offset)
            .map_or(offset, |c| offset + c.len_utf8())
    }

    /// Inicio del carácter anterior; el inicio del texto se queda donde está.
    #[must_use]
    pub fn prev_boundary(&self, offset: ByteOffset) -> ByteOffset {
        self.char_before(offset)
            .map_or(offset, |c| offset.saturating_sub(c.len_utf8()))
    }

    /// Caracteres entre el inicio de la línea y `offset`.
    #[must_use]
    pub fn column_of(&self, offset: ByteOffset) -> usize {
        let offset = offset.min(self.len()).get();
        let line = self.rope.byte_to_line(offset);
        self.rope.byte_to_char(offset) - self.rope.line_to_char(line)
    }

    /// Offset de la columna `column` (en caracteres), sin pasar del fin de línea.
    #[must_use]
    pub fn offset_at_column(&self, line: usize, column: usize) -> ByteOffset {
        let line_start = self.rope.byte_to_char(self.line_start(line).get());
        let line_end = self.rope.byte_to_char(self.line_end(line).get());
        let target = line_start.saturating_add(column).min(line_end);
        ByteOffset::new(self.rope.char_to_byte(target))
    }

    /// Texto de la línea sin su `\n`; sin copiar si está en un solo trozo del rope.
    #[must_use]
    pub fn line_text(&self, line: usize) -> Cow<'_, str> {
        self.lines_text(line..line + 1)
    }

    /// Texto seguido de las líneas `lines`, sin el `\n` de la última; vacío
    /// si el rango lo está. Sin copiar si está en un solo trozo del rope.
    #[must_use]
    pub fn lines_text(&self, lines: Range<usize>) -> Cow<'_, str> {
        if lines.is_empty() {
            return Cow::Borrowed("");
        }
        let (first, last) = (lines.start.min(self.last_line()), lines.end - 1);
        let start = self.line_start(first).get();
        if first == last
            && let Some(line) = self.line_in_chunk(start)
        {
            return Cow::Borrowed(line);
        }
        self.slice(start..self.line_end(last).get())
    }

    /// La línea que empieza en `start`, si cae en el trozo del rope de
    /// `start`: así no hace falta otro recorrido para buscar su final.
    fn line_in_chunk(&self, start: usize) -> Option<&str> {
        let (chunk, chunk_byte, ..) = self.rope.get_chunk_at_byte(start)?;
        let rest = chunk.get(start - chunk_byte..)?;
        match rest.find('\n') {
            Some(end) => rest.get(..end),
            None if chunk_byte + chunk.len() == self.rope.len_bytes() => Some(rest),
            None => None,
        }
    }

    /// # Errors
    /// Si el rango está invertido, se sale del texto o parte un carácter.
    pub fn text_in(&self, range: Range<ByteOffset>) -> Result<String, BufferError> {
        self.check_range(&range)?;
        Ok(self.slice(range.start.get()..range.end.get()).into_owned())
    }

    /// Sin copiar si `bytes` cae en un solo trozo del rope, lo que se sabe
    /// con un solo recorrido.
    pub(super) fn slice(&self, bytes: Range<usize>) -> Cow<'_, str> {
        if let Some((chunk, chunk_byte, ..)) = self.rope.get_chunk_at_byte(bytes.start)
            && let Some(text) = chunk.get(bytes.start - chunk_byte..bytes.end - chunk_byte)
        {
            return Cow::Borrowed(text);
        }
        Cow::Owned(self.rope.byte_slice(bytes).to_string())
    }
}
