//! `:s`, con el patrón y el reemplazo que recuerda el motor.

use nota_buffer::{Buffer, Selections};

use super::Vim;
use crate::search::{Case, Pattern, SearchError};
use crate::substitute::{self, Replacement, SubstituteCommand};
use crate::{Action, CommandError, KeyOutcome, VimError};

/// Lo que repite un `:s` sin patrón.
#[derive(Clone, Debug)]
pub(super) struct LastSubstitute {
    pattern: Pattern,
    replacement: Replacement,
}

impl Vim {
    pub(super) fn substitute(
        &mut self,
        command: &SubstituteCommand,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<KeyOutcome, VimError> {
        let current = buffer.line_of(selections.primary().head);
        let lines = match command.range {
            Some(range) => range.resolve(current, buffer.last_line()),
            None => Ok(current..=current),
        };
        let parts = lines.and_then(|lines| Ok((lines, self.substitution_parts(command)?)));
        let (lines, (remembered, used)) = match parts {
            Ok(parts) => parts,
            Err(error) => return Ok(KeyOutcome::InvalidCommand(error)),
        };
        self.search.pattern = Some(remembered.pattern.clone());
        self.search.highlight = true;
        let applied = substitute::apply(
            buffer,
            &used,
            &remembered.replacement,
            lines,
            command.flags.global,
        )?;
        self.last_substitute = Some(remembered);
        Ok(match applied {
            Some(applied) => {
                *selections = applied.selections;
                KeyOutcome::Substituted(applied.summary)
            }
            None if command.flags.quiet => KeyOutcome::Executed(Action::SubmitCommand),
            None => {
                let source = used.source().to_owned();
                KeyOutcome::InvalidCommand(SearchError::NotFound(source).into())
            }
        })
    }

    /// Como vim, `:s//x/` usa el último patrón (de búsqueda o de `:s`) y `:s`
    /// solo repite el último `:s`. Devuelve lo que se recuerda y el patrón que
    /// se usa ahora: `i` e `I` valen solo para este `:s`.
    fn substitution_parts(
        &mut self,
        command: &SubstituteCommand,
    ) -> Result<(LastSubstitute, Pattern), CommandError> {
        let remembered = match &command.new {
            None => self
                .last_substitute
                .clone()
                .ok_or(SearchError::NoPrevious)?,
            Some(new) => LastSubstitute {
                pattern: self.typed_pattern(&new.pattern)?,
                replacement: Replacement::parse(&new.replacement)?,
            },
        };
        let used = match command.flags.case {
            Some(case) => remembered.pattern.with_case(case)?,
            None => remembered.pattern.clone(),
        };
        Ok((remembered, used))
    }

    /// Vacío es el último patrón usado.
    fn typed_pattern(&mut self, source: &str) -> Result<Pattern, SearchError> {
        if source.is_empty() {
            return self.search.pattern.clone().ok_or(SearchError::NoPrevious);
        }
        self.search.history.push(source);
        Pattern::compile(source, Case::Smart)
    }
}
