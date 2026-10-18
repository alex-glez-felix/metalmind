//! `:s`, con el patrón y el reemplazo que recuerda el motor.

use std::ops::RangeInclusive;

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
    /// Dentro de `:g`, que el patrón no esté en una línea no es error y los
    /// reemplazos se suman para un solo aviso al final.
    pub(super) fn substitute(
        &mut self,
        command: &SubstituteCommand,
        lines: RangeInclusive<usize>,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<KeyOutcome, VimError> {
        let (remembered, used) = match self.substitution_parts(command) {
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
        Ok(match (applied, &mut self.global) {
            (Some(applied), Some(run)) => {
                *selections = applied.selections;
                run.substituted.replacements += applied.summary.replacements;
                run.substituted.lines += applied.summary.lines;
                KeyOutcome::Executed(Action::SubmitCommand)
            }
            (Some(applied), None) => {
                *selections = applied.selections;
                KeyOutcome::Substituted(applied.summary)
            }
            (None, Some(_)) => KeyOutcome::Executed(Action::SubmitCommand),
            (None, None) if command.flags.quiet => KeyOutcome::Executed(Action::SubmitCommand),
            (None, None) => {
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

    /// Vacío es el último patrón usado. Si se repite el último, como en cada
    /// línea de `:g/x/s/x/y/`, no se vuelve a compilar.
    pub(super) fn typed_pattern(&mut self, source: &str) -> Result<Pattern, SearchError> {
        if source.is_empty() {
            return self.search.pattern.clone().ok_or(SearchError::NoPrevious);
        }
        self.search.history.push(source);
        match &self.search.pattern {
            Some(last) if last.is(source, Case::Smart) => Ok(last.clone()),
            _ => Pattern::compile(source, Case::Smart),
        }
    }
}
