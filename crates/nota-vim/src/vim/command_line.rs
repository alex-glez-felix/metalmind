//! La línea donde se escribe tras `:`, `/` o `?`: editarla, recorrer su
//! historial y enviarla.

use nota_buffer::{Buffer, Selections};

use super::Vim;
use crate::ex::ExCommand;
use crate::history::History;
use crate::register::Clipboard;
use crate::{Action, KeyOutcome, Mode, Prompt, VimError};

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

    /// Salir sin enviar también cancela el operador que esperaba a `/`; si
    /// se buscaba desde Visual, se vuelve a la selección.
    pub(super) fn close_command_line(&mut self) {
        let visual = self
            .awaiting_search
            .take()
            .and_then(|awaiting| awaiting.visual);
        self.mode = visual.map_or(Mode::Normal, Mode::Visual);
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
        clipboard: &mut dyn Clipboard,
    ) -> Result<KeyOutcome, VimError> {
        self.mode = Mode::Normal;
        let line = std::mem::take(&mut self.command_line);
        match self.prompt {
            Prompt::Search { backward } => {
                self.submit_search(backward, &line, buffer, selections, clipboard)
            }
            Prompt::Command => {
                self.command_history.push(&line);
                self.run_ex(&line, buffer, selections)
            }
        }
    }

    fn run_ex(
        &mut self,
        line: &str,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<KeyOutcome, VimError> {
        Ok(match ExCommand::parse(line) {
            Ok(None) => KeyOutcome::Executed(Action::SubmitCommand),
            Ok(Some(ExCommand::Host(command))) => KeyOutcome::Command(command),
            Ok(Some(ExCommand::Substitute(command))) => {
                return self.substitute(&command, buffer, selections);
            }
            Ok(Some(ExCommand::ClearSearchHighlight)) => {
                self.search.highlight = false;
                KeyOutcome::Executed(Action::ClearSearchHighlight)
            }
            Err(error) => KeyOutcome::InvalidCommand(error),
        })
    }

    fn history_mut(&mut self) -> &mut History {
        match self.prompt {
            Prompt::Command => &mut self.command_history,
            Prompt::Search { .. } => &mut self.search.history,
        }
    }
}
