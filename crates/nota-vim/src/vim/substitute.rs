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
        let (
            lines,
            LastSubstitute {
                pattern,
                replacement,
            },
        ) = match parts {
            Ok(parts) => parts,
            Err(error) => return Ok(KeyOutcome::InvalidCommand(error)),
        };
        self.search.pattern = Some(pattern.clone());
        self.search.highlight = true;
        let applied =
            substitute::apply(buffer, &pattern, &replacement, lines, command.flags.global)?;
        let source = pattern.source().to_owned();
        self.last_substitute = Some(LastSubstitute {
            pattern,
            replacement,
        });
        Ok(match applied {
            Some(applied) => {
                *selections = applied.selections;
                KeyOutcome::Substituted(applied.summary)
            }
            None if command.flags.quiet => KeyOutcome::Executed(Action::SubmitCommand),
            None => KeyOutcome::InvalidCommand(SearchError::NotFound(source).into()),
        })
    }

    /// Como vim, `:s//x/` usa el último patrón (de búsqueda o de `:s`) y `:s`
    /// solo repite el último `:s`; `i` e `I` lo recompilan.
    fn substitution_parts(
        &mut self,
        command: &SubstituteCommand,
    ) -> Result<LastSubstitute, CommandError> {
        let case = command.flags.case;
        let Some(new) = &command.new else {
            let last = self
                .last_substitute
                .clone()
                .ok_or(SearchError::NoPrevious)?;
            return Ok(LastSubstitute {
                pattern: recompiled(last.pattern, case)?,
                replacement: last.replacement,
            });
        };
        let pattern = if new.pattern.is_empty() {
            let last = self.search.pattern.clone().ok_or(SearchError::NoPrevious)?;
            recompiled(last, case)?
        } else {
            self.search.history.push(&new.pattern);
            Pattern::compile(&new.pattern, case.unwrap_or(Case::Smart))?
        };
        Ok(LastSubstitute {
            pattern,
            replacement: Replacement::parse(&new.replacement)?,
        })
    }
}

fn recompiled(pattern: Pattern, case: Option<Case>) -> Result<Pattern, SearchError> {
    match case {
        Some(case) => pattern.with_case(case),
        None => Ok(pattern),
    }
}
