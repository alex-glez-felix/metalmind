use nota_buffer::{Buffer, BufferError};

use crate::span::Region;

/// Contenido del registro sin nombre.
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
}
