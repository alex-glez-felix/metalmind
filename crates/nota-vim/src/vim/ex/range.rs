//! De un rango escrito a líneas: dónde está el cursor, qué dicen las marks
//! y qué encuentran los patrones.

use std::ops::RangeInclusive;

use nota_buffer::{Buffer, Selections};

use super::Failure;
use super::lines::keep_column;
use crate::ex::{Address, Base, LineRange};
use crate::mark::MarkName;
use crate::search::Query;
use crate::vim::Vim;
use crate::{CommandError, SearchError};

/// Las líneas de un comando sin rango.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DefaultRange {
    CurrentLine,
    /// `:g` y `:sort`.
    Whole,
}

/// Líneas desde 1, como en vim: `0` es antes de la primera y puede pasar del
/// final hasta que se revise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Resolved {
    pub first: usize,
    pub last: usize,
    /// Cuántas direcciones se escribieron, hasta 2.
    pub given: usize,
}

/// Un rango que cabe en el texto, desde 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Lines {
    pub first: usize,
    pub last: usize,
    pub given: usize,
}

impl Resolved {
    /// Recortado al texto, sin revisar el orden.
    pub fn clamped(self, buffer: &Buffer) -> Lines {
        let clamp = |line: usize| line.clamp(1, buffer.line_count());
        Lines {
            first: clamp(self.first),
            last: clamp(self.last),
            given: self.given,
        }
    }

    /// Como vim, la línea 0 vale como la 1.
    ///
    /// # Errors
    /// Si se sale del texto (E16) o va hacia atrás (E493).
    pub fn checked(self, buffer: &Buffer) -> Result<Lines, CommandError> {
        if self.last > buffer.line_count() || self.first > buffer.line_count() {
            return Err(CommandError::InvalidRange);
        }
        if self.first > self.last {
            return Err(CommandError::BackwardsRange);
        }
        Ok(Lines {
            first: self.first.max(1),
            last: self.last.max(1),
            given: self.given,
        })
    }
}

impl Lines {
    pub fn zero_based(self) -> RangeInclusive<usize> {
        self.first - 1..=self.last - 1
    }

    /// `:d 3`: `count` líneas desde la última del rango, sin pasar del final.
    pub fn counted(self, count: Option<usize>, buffer: &Buffer) -> Self {
        let Some(count) = count else {
            return self;
        };
        Self {
            first: self.last,
            last: self.last.saturating_add(count - 1).min(buffer.line_count()),
            given: self.given + 1,
        }
    }
}

impl Vim {
    /// Como vim, cuentan las dos últimas direcciones, y tras `;` el cursor
    /// va a la anterior, aunque el comando falle después.
    pub(super) fn resolve_range(
        &mut self,
        range: Option<&LineRange>,
        default: DefaultRange,
        buffer: &Buffer,
        selections: &mut Selections,
    ) -> Result<Resolved, Failure> {
        let mut cursor = buffer.line_of(selections.primary().head) + 1;
        let steps = match (range, default) {
            (None, DefaultRange::CurrentLine) => {
                return Ok(Resolved {
                    first: cursor,
                    last: cursor,
                    given: 0,
                });
            }
            (None, DefaultRange::Whole) | (Some(LineRange::Whole), _) => {
                return Ok(Resolved {
                    first: 1,
                    last: buffer.line_count(),
                    given: usize::from(range.is_some()) * 2,
                });
            }
            (Some(LineRange::Addresses(steps)), _) => steps,
        };
        let (mut first, mut last) = (cursor, cursor);
        for step in steps {
            first = last;
            last = match &step.address {
                Some(address) => self.resolve_address(address, cursor, buffer)?,
                None => cursor,
            };
            if step.moves_cursor {
                cursor = last.clamp(1, buffer.line_count());
                *selections =
                    Selections::single(keep_column(buffer, selections.primary(), cursor - 1));
            }
        }
        if steps.len() == 1 {
            first = last;
        }
        Ok(Resolved {
            first,
            last,
            given: steps.len().min(2),
        })
    }

    /// La línea, desde 1, debajo de la que van `:m` y `:t`.
    pub(super) fn resolve_destination(
        &mut self,
        address: &Address,
        buffer: &Buffer,
        selections: &Selections,
    ) -> Result<usize, Failure> {
        let cursor = buffer.line_of(selections.primary().head) + 1;
        let below = self.resolve_address(address, cursor, buffer)?;
        if below > buffer.line_count() {
            return Err(CommandError::InvalidAddress.into());
        }
        Ok(below)
    }

    /// Desde 1. Una búsqueda que sigue a otra empieza donde quedó esa.
    fn resolve_address(
        &mut self,
        address: &Address,
        cursor: usize,
        buffer: &Buffer,
    ) -> Result<usize, Failure> {
        let mut line = cursor;
        for term in &address.terms {
            let base = match &term.base {
                Base::Current => cursor,
                Base::Last => buffer.line_count(),
                Base::Number(number) => *number,
                Base::Mark(mark) => self.mark_line(*mark, buffer)? + 1,
                Base::Search { pattern, backward } => {
                    self.search_line(pattern, *backward, line, buffer)? + 1
                }
            };
            line = base
                .checked_add_signed(term.offset)
                .ok_or(CommandError::InvalidRange)?;
        }
        Ok(line)
    }

    /// `'a`–`'z`, `''` y `'<`, `'>` de la última selección Visual; desde 0.
    fn mark_line(&self, mark: char, buffer: &Buffer) -> Result<usize, CommandError> {
        let line = match mark {
            '<' | '>' => self
                .last_visual
                .as_ref()
                .and_then(|visual| visual.lines(buffer))
                .map(|(start, end)| if mark == '<' { start } else { end }),
            _ => MarkName::parse(mark)
                .and_then(|name| self.marks.get(buffer, name))
                .map(|offset| buffer.line_of(offset)),
        };
        line.ok_or(CommandError::MarkNotSet)
    }

    /// La primera línea con el patrón después de `from` (o antes, con `?`),
    /// dando la vuelta; desde 0. Como en vim, queda como última búsqueda.
    fn search_line(
        &mut self,
        source: &str,
        backward: bool,
        from: usize,
        buffer: &Buffer,
    ) -> Result<usize, SearchError> {
        let pattern = self.typed_pattern(source)?;
        self.search.remember(&Query {
            pattern: pattern.clone(),
            backward,
        });
        let count = buffer.line_count();
        let from = from.min(count);
        let found = if backward {
            let before = from.saturating_sub(1);
            pattern
                .last_matching_line(buffer, 0..before)
                .or_else(|| pattern.last_matching_line(buffer, before..count))
        } else {
            let mut after = pattern.matching_lines(buffer, from..count);
            after
                .next()
                .or_else(|| pattern.matching_lines(buffer, 0..from).next())
        };
        found.ok_or_else(|| SearchError::NotFound(pattern.source().to_owned()))
    }
}
