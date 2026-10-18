//! `:g/patrón/comando`, `:g!` y `:v`: marca las líneas y corre el comando
//! en cada una que siga existiendo, como `ex_global` de vim.

use nota_buffer::{Buffer, Selection, Selections};

use super::lines::{keep_column, line_start_cursor};
use super::range::Lines;
use super::{Failure, done};
use crate::ex::{GlobalCommand, LineRange};
use crate::lines::Want;
use crate::register::Clipboard;
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
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<KeyOutcome, Failure> {
        let pattern = self.typed_pattern(&command.pattern)?;
        self.search.pattern = Some(pattern.clone());
        self.search.highlight = true;
        if self.global.is_some() {
            let whole = lines.first == 1 && lines.last == buffer.line_count();
            if range.is_some() && !whole {
                return Err(CommandError::RecursiveGlobal.into());
            }
            return self.global_here(command, &pattern, buffer, selections, clipboard);
        }
        let marked = marked_lines(buffer, &pattern, lines, command.inverted);
        if marked.is_empty() {
            return Ok(KeyOutcome::GlobalMissed(GlobalMiss {
                pattern: pattern.source().to_owned(),
                inverted: command.inverted,
            }));
        }
        buffer.mark_lines(marked);
        self.global = Some(GlobalRun {
            substituted: Substitution::default(),
        });
        self.ex_depth += 1;
        buffer.begin_undo_group();
        let ran = self.run_on_marked(&command.command, buffer, selections, clipboard);
        buffer.end_undo_group();
        buffer.clear_marked_lines();
        self.ex_depth -= 1;
        let substituted = self.global.take().map(|run| run.substituted);
        ran?;
        match substituted {
            // Como vim, el cursor va al primer no blanco si algún `:s` cambió algo.
            Some(substituted) if substituted.replacements > 0 => {
                let line = buffer.line_of(selections.primary().head);
                *selections = Selections::single(line_start_cursor(buffer, line));
                Ok(KeyOutcome::Substituted(substituted))
            }
            _ => Ok(done()),
        }
    }

    /// Cada línea marcada empieza en la columna 0 sin cambiar la columna
    /// buscada; el primer error corta, también el de una búsqueda dentro de
    /// un `:normal`.
    fn run_on_marked(
        &mut self,
        command: &str,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<(), Failure> {
        while let Some(line) = buffer.pop_marked_line() {
            let goal = Want::of(buffer, selections.primary(), true, self.viewport.columns()).goal();
            *selections = Selections::single(Selection {
                goal,
                ..Selection::cursor(buffer.line_start(line))
            });
            match self.run_global_command(command, buffer, selections, clipboard)? {
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
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<KeyOutcome, Failure> {
        let line = buffer.line_of(selections.primary().head);
        let matches = pattern
            .matching_lines(buffer, line..line + 1)
            .next()
            .is_some();
        if matches == command.inverted {
            return Ok(done());
        }
        self.run_global_command(&command.command, buffer, selections, clipboard)
    }

    /// Sin comando, solo va a la línea, como el `:p` de vim.
    fn run_global_command(
        &mut self,
        command: &str,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<KeyOutcome, Failure> {
        if command.trim().is_empty() {
            let line = buffer.line_of(selections.primary().head);
            *selections = Selections::single(keep_column(
                buffer,
                self.viewport.columns(),
                selections.primary(),
                line,
            ));
            return Ok(done());
        }
        Ok(self.run_ex(command, buffer, selections, clipboard)?)
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
