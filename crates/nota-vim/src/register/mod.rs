//! Registros de vim: qué guardan y a cuál va cada yank y borrado.

mod clipboard;
mod name;
mod store;

use nota_buffer::{Buffer, BufferError};

pub(crate) use clipboard::Detached;
pub use clipboard::{Clipboard, SystemRegister};
pub(crate) use name::RegisterName;
pub(crate) use store::{Registers, Write};

use crate::span::Region;

/// Contenido de un registro.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Register {
    /// En registros por líneas, sin el `\n` final; en uno por bloque, cada
    /// línea del bloque separada por `\n`.
    pub text: String,
    pub shape: Shape,
}

/// Cómo se pega un registro, como el `y_type` de vim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shape {
    Chars,
    Lines,
    /// Cada línea va a la misma columna de una línea distinta. `width` es
    /// el ancho del bloque en columnas de pantalla menos uno (`y_width`).
    Block {
        width: usize,
    },
}

impl Register {
    pub fn chars(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            shape: Shape::Chars,
        }
    }

    pub fn lines(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            shape: Shape::Lines,
        }
    }

    pub fn is_linewise(&self) -> bool {
        self.shape == Shape::Lines
    }

    /// Con varias selecciones, cada trozo va en su propia línea.
    pub fn from_regions<'a>(
        buffer: &Buffer,
        regions: impl IntoIterator<Item = &'a Region>,
    ) -> Result<Self, BufferError> {
        let mut pieces = Vec::new();
        let mut shape = Shape::Lines;
        for region in regions {
            let range = match region {
                Region::Chars(range) => {
                    shape = Shape::Chars;
                    range.clone()
                }
                Region::Lines { first, last } => buffer.line_start(*first)..buffer.line_end(*last),
            };
            pieces.push(buffer.text_in(range)?);
        }
        Ok(Self {
            text: pieces.join("\n"),
            shape,
        })
    }

    /// `"A`, como `op_yank_reg` de vim: lo nuevo por líneas vuelve el
    /// resultado por líneas; si no, se queda la forma de lo que había. Tras
    /// uno por caracteres, lo nuevo sigue en su última línea; tras uno por
    /// líneas o por bloque, empieza en otra.
    pub fn append(self, more: &Self) -> Self {
        let shape = if more.is_linewise() {
            Shape::Lines
        } else {
            self.shape
        };
        let text = if shape == Shape::Chars {
            self.text + &more.text
        } else {
            format!("{}\n{}", self.text, more.text)
        };
        Self { text, shape }
    }

    /// Como nvim, un registro por líneas termina en `\n` en el portapapeles;
    /// uno por bloque va con sus líneas, sin el `\n` final.
    pub fn to_clipboard(&self) -> String {
        if self.is_linewise() {
            format!("{}\n", self.text)
        } else {
            self.text.clone()
        }
    }

    /// Lo que termina en `\n` se pega por líneas, como en nvim.
    pub fn from_clipboard(text: String) -> Option<Self> {
        if text.is_empty() {
            return None;
        }
        let register = match text.strip_suffix('\n') {
            Some(lines) => Self::lines(lines),
            None => Self::chars(text),
        };
        Some(register)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(text: &str) -> Register {
        Register::chars(text)
    }

    fn lines(text: &str) -> Register {
        Register::lines(text)
    }

    fn block(text: &str, width: usize) -> Register {
        Register {
            text: text.to_owned(),
            shape: Shape::Block { width },
        }
    }

    #[test]
    fn appending_joins_chars_and_stacks_lines() {
        assert_eq!(chars("ab").append(&chars("c")), chars("abc"));
        assert_eq!(lines("ab").append(&chars("c")), lines("ab\nc"));
        assert_eq!(chars("ab").append(&lines("c")), lines("ab\nc"));
        assert_eq!(lines("ab").append(&lines("c\nd")), lines("ab\nc\nd"));
    }

    #[test]
    fn appending_to_a_block_keeps_its_shape_unless_lines_come() {
        assert_eq!(
            block("ab\ncd", 1).append(&chars("e")),
            block("ab\ncd\ne", 1)
        );
        assert_eq!(
            block("ab", 1).append(&block("c\nd", 0)),
            block("ab\nc\nd", 1)
        );
        assert_eq!(block("ab", 1).append(&lines("c")), lines("ab\nc"));
        assert_eq!(chars("ab").append(&block("c\nd", 0)), chars("abc\nd"));
        assert_eq!(lines("ab").append(&block("c", 0)), lines("ab\nc"));
        assert_eq!(block("a\nb", 0).to_clipboard(), "a\nb");
    }

    #[test]
    fn the_clipboard_marks_lines_with_a_final_newline() {
        assert_eq!(lines("a\nb").to_clipboard(), "a\nb\n");
        assert_eq!(chars("a\nb").to_clipboard(), "a\nb");
        assert_eq!(Register::from_clipboard("a\n".to_owned()), Some(lines("a")));
        assert_eq!(Register::from_clipboard("a".to_owned()), Some(chars("a")));
        assert_eq!(Register::from_clipboard(String::new()), None);
    }
}
