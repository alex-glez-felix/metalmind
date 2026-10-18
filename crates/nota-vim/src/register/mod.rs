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
    /// En registros por líneas, sin el `\n` final.
    pub text: String,
    pub linewise: bool,
}

impl Register {
    /// Con varias selecciones, cada trozo va en su propia línea.
    pub fn from_regions<'a>(
        buffer: &Buffer,
        regions: impl IntoIterator<Item = &'a Region>,
    ) -> Result<Self, BufferError> {
        let mut pieces = Vec::new();
        let mut linewise = true;
        for region in regions {
            let range = match region {
                Region::Chars(range) => {
                    linewise = false;
                    range.clone()
                }
                Region::Lines { first, last } => buffer.line_start(*first)..buffer.line_end(*last),
            };
            pieces.push(buffer.text_in(range)?);
        }
        Ok(Self {
            text: pieces.join("\n"),
            linewise,
        })
    }

    /// `"A`: lo nuevo va al final. Si alguno de los dos es por líneas, el
    /// resultado también, y lo nuevo empieza en otra línea.
    pub fn append(self, more: &Self) -> Self {
        if self.linewise || more.linewise {
            Self {
                text: format!("{}\n{}", self.text, more.text),
                linewise: true,
            }
        } else {
            Self {
                text: self.text + &more.text,
                linewise: false,
            }
        }
    }

    /// Como nvim, un registro por líneas termina en `\n` en el portapapeles.
    pub fn to_clipboard(&self) -> String {
        if self.linewise {
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
            Some(lines) => Self {
                text: lines.to_owned(),
                linewise: true,
            },
            None => Self {
                text,
                linewise: false,
            },
        };
        Some(register)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(text: &str) -> Register {
        Register {
            text: text.to_owned(),
            linewise: false,
        }
    }

    fn lines(text: &str) -> Register {
        Register {
            text: text.to_owned(),
            linewise: true,
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
    fn the_clipboard_marks_lines_with_a_final_newline() {
        assert_eq!(lines("a\nb").to_clipboard(), "a\nb\n");
        assert_eq!(chars("a\nb").to_clipboard(), "a\nb");
        assert_eq!(Register::from_clipboard("a\n".to_owned()), Some(lines("a")));
        assert_eq!(Register::from_clipboard("a".to_owned()), Some(chars("a")));
        assert_eq!(Register::from_clipboard(String::new()), None);
    }
}
