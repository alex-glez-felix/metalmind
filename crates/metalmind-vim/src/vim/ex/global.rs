//! `:g/patrón/comando`, `:g!` y `:v`: marca las líneas y corre el comando
//! en cada una que siga existiendo, como `ex_global` de vim.

use metalmind_buffer::{Buffer, Selection, Selections};

use super::lines::{keep_column, line_start_cursor};
use super::range::Lines;
use super::{Failure, done};
use crate::Target;
use crate::ex::{GlobalCommand, LineRange};
use crate::lines::Want;
use crate::search::Pattern;
use crate::vim::Vim;
use crate::{CommandError, GlobalMiss, KeyOutcome, Substitution};

/// Un `:g` en curso.
#[derive(Debug)]
pub(in crate::vim) struct GlobalRun {
    /// Lo que reemplazaron los `:s` de todas las líneas, para un solo aviso.
    pub substituted: Substitution,
}

impl Vim {
    /// Todo es un solo paso de undo. El patrón queda como última búsqueda.
    pub(super) fn global(
        &mut self,
        command: &GlobalCommand,
        range: Option<&LineRange>,
        lines: Lines,
        t: &mut Target<'_>,
    ) -> Result<KeyOutcome, Failure> {
        let pattern = self.typed_pattern(&command.pattern)?;
        self.search.pattern = Some(pattern.clone());
        self.search.highlight = true;
        if self.global.is_some() {
            let whole = lines.first == 1 && lines.last == t.buffer.line_count();
            if range.is_some() && !whole {
                return Err(CommandError::RecursiveGlobal.into());
            }
            return self.global_here(command, &pattern, t);
        }
        let marked = marked_lines(t.buffer, &pattern, lines, command.inverted);
        if marked.is_empty() {
            return Ok(KeyOutcome::GlobalMissed(GlobalMiss {
                pattern: pattern.source().to_owned(),
                inverted: command.inverted,
            }));
        }
        self.jumped(t)?;
        t.buffer.mark_lines(marked);
        self.global = Some(GlobalRun {
            substituted: Substitution::default(),
        });
        self.ex_depth += 1;
        t.buffer.begin_undo_group();
        let ran = self.run_on_marked(&command.command, t);
        t.buffer.end_undo_group();
        t.buffer.clear_marked_lines();
        self.ex_depth -= 1;
        let substituted = self.global.take().map(|run| run.substituted);
        ran?;
        match substituted {
            // Como vim, el cursor va al primer no blanco si algún `:s` cambió algo.
            Some(substituted) if substituted.replacements > 0 => {
                let line = t.buffer.line_of(t.selections.primary().head);
                *t.selections = Selections::single(line_start_cursor(t.buffer, line));
                Ok(KeyOutcome::Substituted(substituted))
            }
            _ => Ok(done()),
        }
    }

    /// Cada línea marcada empieza en la columna 0 sin cambiar la columna
    /// buscada; el primer error corta, también el de una búsqueda dentro de
    /// un `:normal`.
    fn run_on_marked(&mut self, command: &str, t: &mut Target<'_>) -> Result<(), Failure> {
        while let Some(line) = t.buffer.pop_marked_line() {
            let goal = Want::of(
                t.buffer,
                t.selections.primary(),
                true,
                t.pane.viewport.columns(),
            )
            .goal();
            *t.selections = Selections::single(Selection {
                goal,
                ..Selection::cursor(t.buffer.line_start(line))
            });
            match self.run_global_command(command, t)? {
                KeyOutcome::InvalidCommand(error) => return Err(error.into()),
                KeyOutcome::SearchFailed(error) => return Err(error.into()),
                _ => {}
            }
        }
        Ok(())
    }

    /// Un `:g` dentro de otro, como en vim, solo mira la línea del cursor.
    fn global_here(
        &mut self,
        command: &GlobalCommand,
        pattern: &Pattern,
        t: &mut Target<'_>,
    ) -> Result<KeyOutcome, Failure> {
        let line = t.buffer.line_of(t.selections.primary().head);
        let matches = pattern
            .matching_lines(t.buffer, line..line + 1)
            .next()
            .is_some();
        if matches == command.inverted {
            return Ok(done());
        }
        self.run_global_command(&command.command, t)
    }

    /// Sin comando, solo va a la línea, como el `:p` de vim.
    fn run_global_command(
        &mut self,
        command: &str,
        t: &mut Target<'_>,
    ) -> Result<KeyOutcome, Failure> {
        if command.trim().is_empty() {
            let line = t.buffer.line_of(t.selections.primary().head);
            *t.selections = Selections::single(keep_column(
                t.buffer,
                t.pane.viewport.columns(),
                t.selections.primary(),
                line,
            ));
            return Ok(done());
        }
        Ok(self.run_ex(command, t)?)
    }
}

/// Las líneas de `lines` con el patrón o, invertido, sin él.
fn marked_lines(buffer: &Buffer, pattern: &Pattern, lines: Lines, inverted: bool) -> Vec<usize> {
    let lines = lines.zero_based();
    let range = *lines.start()..lines.end() + 1;
    let matching = pattern.matching_lines(buffer, range.clone());
    if !inverted {
        return matching.collect();
    }
    let mut matching = matching.peekable();
    range
        .filter(|&line| {
            let matched = matching.next_if_eq(&line).is_some();
            !matched
        })
        .collect()
}
