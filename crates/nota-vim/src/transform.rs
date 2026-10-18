use std::ops::Range;

use nota_buffer::{Buffer, ByteOffset, Edit, PerSelection, Selection};

use crate::VimError;
use crate::lines::clamp_normal;
use crate::operator::{Operand, Outcome, content, union};

/// Cambia cada carácter por otro sin tocar el registro: `r` en Visual y los
/// operadores de mayúsculas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Transform {
    Lowercase,
    Uppercase,
    ToggleCase,
    Replace(char),
}

impl Transform {
    fn text(self, text: &str) -> String {
        text.chars()
            .map(|c| if c == '\n' { c } else { self.char(c) })
            .collect()
    }

    fn char(self, c: char) -> char {
        match self {
            Self::Replace(replacement) => replacement,
            Self::Lowercase => single(c.to_lowercase()).unwrap_or(c),
            Self::Uppercase => single(c.to_uppercase()).unwrap_or(c),
            Self::ToggleCase if c.is_lowercase() => single(c.to_uppercase()).unwrap_or(c),
            Self::ToggleCase if c.is_uppercase() => single(c.to_lowercase()).unwrap_or(c),
            Self::ToggleCase => c,
        }
    }

    /// Aplica el cambio a lo que abarca cada operando y deja el cursor al
    /// inicio, como vim. Sin nada que cambiar no abre transacción. Cada
    /// carácter cambia por uno solo y los saltos de línea quedan, así que las
    /// líneas y columnas se conservan aunque cambien los bytes.
    pub(crate) fn apply(
        self,
        buffer: &mut Buffer,
        operands: &PerSelection<Operand>,
    ) -> Result<Outcome, VimError> {
        let starts = operands.map(|operand| {
            let start = match &operand.target {
                Ok(target) => target.start,
                Err(missed) => missed.selection(&operand.selection).head,
            };
            (buffer.line_of(start), buffer.column_of(start))
        });
        let cursors = |buffer: &Buffer| starts.to_selections(|&start| cursor_on(buffer, start));
        if operands.iter().all(|operand| operand.target.is_err()) {
            return Ok(Outcome::Missed(cursors(buffer)));
        }
        let ranges = operands
            .iter()
            .filter_map(|operand| operand.target.as_ref().ok())
            .map(|target| content(buffer, &target.region));
        let edits = self.edits(buffer, union(ranges))?;
        let selections = if edits.is_empty() {
            cursors(buffer)
        } else {
            buffer.begin_transaction(cursors(buffer))?;
            buffer.apply(edits)?;
            let after = cursors(buffer);
            buffer.commit_transaction(after.clone())?;
            after
        };
        Ok(Outcome::Applied {
            selections,
            register: None,
            spans_lines: false,
        })
    }

    /// Solo los rangos donde el texto cambia de verdad.
    fn edits(self, buffer: &Buffer, ranges: Vec<Range<ByteOffset>>) -> Result<Vec<Edit>, VimError> {
        let mut edits = Vec::with_capacity(ranges.len());
        for range in ranges {
            let old = buffer.text_in(range.clone())?;
            let new = self.text(&old);
            if new != old {
                edits.push(Edit::replace(range, new));
            }
        }
        Ok(edits)
    }
}

fn cursor_on(buffer: &Buffer, (line, column): (usize, usize)) -> Selection {
    Selection::cursor(clamp_normal(buffer, buffer.offset_at_column(line, column)))
}

/// Como `utf_toupper` de vim, cambia un carácter por uno solo: si el cambio
/// da varios (`ß` → `SS`), lo deja como está.
fn single(mut chars: impl Iterator<Item = char>) -> Option<char> {
    let c = chars.next()?;
    chars.next().is_none().then_some(c)
}
