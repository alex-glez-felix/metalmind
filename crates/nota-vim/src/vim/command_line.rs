//! La línea donde se escribe tras `:`, `/` o `?`: editarla, recorrer su
//! historial y enviarla.

use nota_buffer::{Buffer, Selections};

use super::Vim;
use crate::history::History;
use crate::{Action, Command, KeyOutcome, Mode, Prompt, VimError};

impl Vim {
    /// Lo escrito tras `:`, `/` o `?`, mientras se está en la línea de comandos.
    #[must_use]
    pub fn command_line(&self) -> Option<&str> {
        (self.mode == Mode::CommandLine).then_some(self.command_line.as_str())
    }

    /// Qué se está escribiendo en la línea de comandos, para su símbolo.
    #[must_use]
    pub fn command_line_prompt(&self) -> Option<Prompt> {
        (self.mode == Mode::CommandLine).then_some(self.prompt)
    }

    pub(super) fn open_command_line(&mut self, prompt: Prompt) {
        self.reset();
        self.command_line.clear();
        self.prompt = prompt;
        self.history_mut().stop_browsing();
        self.mode = Mode::CommandLine;
    }

    /// Salir sin enviar también cancela el operador que esperaba a `/`.
    pub(super) fn close_command_line(&mut self) {
        self.awaiting_search = None;
        self.mode = Mode::Normal;
    }

    pub(super) fn type_in_command_line(&mut self, text: &str) {
        self.command_line.push_str(text);
        self.history_mut().stop_browsing();
    }

    /// Borrar con la línea vacía la cierra, como en vim.
    pub(super) fn command_line_backspace(&mut self) {
        if self.command_line.pop().is_none() {
            self.close_command_line();
        }
        self.history_mut().stop_browsing();
    }

    pub(super) fn recall(&mut self, older: bool) {
        let typed = std::mem::take(&mut self.command_line);
        let history = self.history_mut();
        let recalled = if older {
            history.older(&typed)
        } else {
            history.newer()
        };
        self.command_line = recalled.map_or(typed, str::to_owned);
    }

    pub(super) fn submit_command_line(
        &mut self,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<KeyOutcome, VimError> {
        self.mode = Mode::Normal;
        let line = std::mem::take(&mut self.command_line);
        match self.prompt {
            Prompt::Search { backward } => self.submit_search(backward, &line, buffer, selections),
            Prompt::Command => {
                self.command_history.push(&line);
                Ok(run_command(&line))
            }
        }
    }

    fn history_mut(&mut self) -> &mut History {
        match self.prompt {
            Prompt::Command => &mut self.command_history,
            Prompt::Search { .. } => &mut self.search.history,
        }
    }
}

fn run_command(line: &str) -> KeyOutcome {
    match Command::parse(line) {
        Ok(Some(command)) => KeyOutcome::Command(command),
        Ok(None) => KeyOutcome::Executed(Action::SubmitCommand),
        Err(error) => KeyOutcome::InvalidCommand(error),
    }
}
