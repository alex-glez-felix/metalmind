//! Correr lo escrito tras `:`. Cada comando es un solo paso de undo.

mod global;
mod lines;
mod normal;
mod range;
mod sort;

use nota_buffer::{Buffer, BufferError, Selections};

pub(super) use self::global::GlobalRun;
use self::range::DefaultRange;
use super::Vim;
use crate::ex::{ExCommand, ExLine, ParsedLine};
use crate::register::Clipboard;
use crate::{Action, CommandError, KeyOutcome, SearchError, VimError};

/// Por qué no terminó un comando: lo escrito no vale, o una edición falló,
/// que sería un bug.
#[derive(Debug)]
enum Failure {
    Command(CommandError),
    Vim(VimError),
}

impl From<CommandError> for Failure {
    fn from(error: CommandError) -> Self {
        Self::Command(error)
    }
}

impl From<SearchError> for Failure {
    fn from(error: SearchError) -> Self {
        Self::Command(error.into())
    }
}

impl From<VimError> for Failure {
    fn from(error: VimError) -> Self {
        Self::Vim(error)
    }
}

impl From<BufferError> for Failure {
    fn from(error: BufferError) -> Self {
        Self::Vim(error.into())
    }
}

/// Desde cuántas líneas de `:` una dentro de otra vim da `E169`.
const MAX_COMMAND_DEPTH: usize = 200;

impl Vim {
    /// Corre una línea de `:`; los comandos del host se devuelven para que
    /// los corra quien aloja el editor, salvo dentro de `:g` o `:normal`.
    pub(super) fn run_ex(
        &mut self,
        line: &str,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<KeyOutcome, VimError> {
        if self.command_depth >= MAX_COMMAND_DEPTH {
            return Ok(KeyOutcome::InvalidCommand(CommandError::TooRecursive));
        }
        self.command_depth += 1;
        let outcome = self.run_line(line, buffer, selections, clipboard);
        self.command_depth -= 1;
        outcome
    }

    fn run_line(
        &mut self,
        line: &str,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<KeyOutcome, VimError> {
        let parsed = match ParsedLine::parse(line) {
            Ok(Some(ParsedLine::Engine(parsed))) => parsed,
            Ok(Some(ParsedLine::Host(_))) if self.ex_depth > 0 => {
                let typed = line.trim_matches([' ', ':']).to_owned();
                let error = CommandError::HostCommandInside(typed);
                return Ok(KeyOutcome::InvalidCommand(error));
            }
            Ok(Some(ParsedLine::Host(invocation))) => return Ok(KeyOutcome::Command(invocation)),
            Ok(None) => return Ok(done()),
            Err(error) => return Ok(KeyOutcome::InvalidCommand(error)),
        };
        match self.run_parsed(parsed, buffer, selections, clipboard) {
            Ok(outcome) => Ok(outcome),
            Err(Failure::Command(error)) => Ok(KeyOutcome::InvalidCommand(error)),
            Err(Failure::Vim(error)) => Err(error),
        }
    }

    fn run_parsed(
        &mut self,
        parsed: ExLine,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<KeyOutcome, Failure> {
        let range = parsed.range.as_ref();
        let default = match parsed.command {
            ExCommand::Global(_) | ExCommand::Sort(_) => DefaultRange::Whole,
            _ => DefaultRange::CurrentLine,
        };
        let resolved = self.resolve_range(range, default, buffer, selections)?;
        let lines = match parsed.command {
            // Como vim, un rango solo no pasa del final.
            ExCommand::GoTo => resolved.clamped(buffer),
            _ => resolved.checked(buffer)?,
        };
        match parsed.command {
            ExCommand::GoTo => {
                lines::go_to(lines.last - 1, self.viewport.columns(), buffer, selections);
            }
            ExCommand::Substitute(command) => {
                return Ok(self.substitute(&command, lines.zero_based(), buffer, selections)?);
            }
            ExCommand::ClearSearchHighlight => {
                self.search.highlight = false;
                return Ok(KeyOutcome::Executed(Action::ClearSearchHighlight));
            }
            ExCommand::Delete(args) => {
                let lines = lines.counted(args.count, buffer).zero_based();
                self.delete_lines(lines, args.register, buffer, selections, clipboard)?;
            }
            ExCommand::Yank(args) => {
                let lines = lines.counted(args.count, buffer).zero_based();
                self.yank_lines(&lines, args.register, buffer, clipboard);
            }
            ExCommand::Move(address) => {
                let below = self.resolve_destination(&address, buffer, selections)?;
                lines::move_lines(
                    lines.zero_based(),
                    below,
                    self.viewport.columns(),
                    buffer,
                    selections,
                )?;
            }
            ExCommand::Copy(address) => {
                let below = self.resolve_destination(&address, buffer, selections)?;
                lines::copy_lines(
                    lines.zero_based(),
                    below,
                    self.viewport.columns(),
                    buffer,
                    selections,
                )?;
            }
            ExCommand::Join { spaces, count } => {
                self.join_lines(lines.counted(count, buffer), spaces, buffer, selections)?;
            }
            ExCommand::Shift {
                shift,
                levels,
                count,
            } => {
                let lines = lines.counted(count, buffer).zero_based();
                self.shift_lines(lines, shift, levels, buffer, selections)?;
            }
            ExCommand::Sort(options) => {
                self.sort_lines(lines.zero_based(), &options, buffer, selections)?;
            }
            ExCommand::Normal(keys) => {
                let lines = range.map(|_| lines.zero_based());
                return self.normal(&keys, lines, buffer, selections, clipboard);
            }
            ExCommand::Global(command) => {
                return self.global(&command, range, lines, buffer, selections, clipboard);
            }
        }
        Ok(done())
    }
}

fn done() -> KeyOutcome {
    KeyOutcome::Executed(Action::SubmitCommand)
}
