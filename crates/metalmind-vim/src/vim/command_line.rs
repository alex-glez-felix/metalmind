//! La línea donde se escribe tras `:`, `/` o `?`: editarla, recorrer su
//! historial y enviarla.

use std::ops::Range;

use super::Vim;
use crate::argument::{ArgumentWord, argument_words};
use crate::command::argument_start;
use crate::command_names::names_starting_with;
use crate::ex::{LineRange, skip_blanks};
use crate::history::History;
use crate::{ActionName, KeyOutcome, Mode, Prompt, VimError};
use crate::{CommandName, Target};

/// La palabra que se escribe en el argumento de un comando ajeno, para
/// autocompletarla.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypedArgument {
    pub action: ActionName,
    /// La última, con su rango en bytes de la línea; tras un espacio, vacía.
    pub word: ArgumentWord,
}

/// Lo que se escribe tras `:` fuera del argumento de un comando, para
/// autocompletarlo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypedCommand {
    /// Nada todavía: lo enviado antes, de lo más nuevo a lo más viejo.
    Empty { history: Vec<String> },
    /// El nombre del comando, tras su rango si lo hay: `range` son sus
    /// bytes en la línea y `names`, los que empiezan con él.
    Name {
        range: Range<usize>,
        names: Vec<CommandName>,
    },
}

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

    /// Lo que se escribe tras `:` cuando es el argumento de un comando
    /// ajeno: `:focus proyectos/ #ru` da `workspace::focus` y `#ru`.
    #[must_use]
    pub fn typed_argument(&self) -> Option<TypedArgument> {
        if self.prompt != Prompt::Command {
            return None;
        }
        let line = self.command_line()?;
        let (command, start) = argument_start(line, self.keymap.commands())?;
        let word = argument_words(&line[start..])
            .pop()
            .filter(|word| start + word.range.end == line.len())
            .map_or_else(
                || ArgumentWord {
                    range: line.len()..line.len(),
                    text: String::new(),
                    literal_start: false,
                },
                |word| ArgumentWord {
                    range: start + word.range.start..line.len(),
                    ..word
                },
            );
        Some(TypedArgument {
            action: command.action,
            word,
        })
    }

    /// Cambia la palabra de [`Vim::typed_argument`] por `text`, ya escrito
    /// con sus escapes. Si no se escribe un argumento, no hace nada.
    pub fn complete_argument(&mut self, text: &str) {
        let Some(typed) = self.typed_argument() else {
            return;
        };
        self.command_line.truncate(typed.word.range.start);
        self.type_in_command_line(text);
    }

    /// Lo que se escribe tras `:` si es nada o el nombre de un comando:
    /// `'<,'>s` da `s`. `None` si no hay nada que ofrecer.
    #[must_use]
    pub fn typed_command(&self) -> Option<TypedCommand> {
        if self.prompt != Prompt::Command {
            return None;
        }
        let line = self.command_line()?;
        if line.is_empty() {
            let history: Vec<String> = self
                .command_history
                .newest_first()
                .map(str::to_owned)
                .collect();
            return (!history.is_empty()).then_some(TypedCommand::Empty { history });
        }
        let typed = line.trim_start_matches([' ', ':']);
        let (_, rest) = LineRange::parse(typed).ok()?;
        let name = skip_blanks(rest);
        let letters = name.strip_suffix('!').unwrap_or(name);
        if letters.is_empty() || !letters.chars().all(|c| c.is_ascii_alphabetic()) {
            return None;
        }
        let names = names_starting_with(name, self.keymap.commands());
        (!names.is_empty()).then(|| TypedCommand::Name {
            range: line.len() - name.len()..line.len(),
            names,
        })
    }

    /// Cambia lo de [`Vim::typed_command`] por `text`: el nombre de un
    /// comando o una línea entera del historial. Si no hay nada que
    /// ofrecer, no hace nada.
    pub fn complete_command(&mut self, text: &str) {
        let start = match self.typed_command() {
            Some(TypedCommand::Empty { .. }) => 0,
            Some(TypedCommand::Name { range, .. }) => range.start,
            None => return,
        };
        self.command_line.truncate(start);
        self.type_in_command_line(text);
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
        t: &mut Target<'_>,
    ) -> Result<KeyOutcome, VimError> {
        self.mode = Mode::Normal;
        let line = std::mem::take(&mut self.command_line);
        match self.prompt {
            Prompt::Search { backward } => self.submit_search(backward, &line, t),
            // Como vim, `@:` dentro de la línea repite la anterior, no esta.
            Prompt::Command => {
                self.command_history.push(&line);
                let outcome = self.run_ex(&line, t);
                if !line.is_empty() {
                    self.last_command_line = Some(line);
                }
                outcome
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
