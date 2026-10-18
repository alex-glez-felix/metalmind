mod awaiting;
mod command_line;
mod find;
mod insert;
mod operate;
mod repeat;
mod search;
mod substitute;

use nota_buffer::{Buffer, Selection, Selections};

use self::awaiting::Awaiting;
use self::search::{AwaitingSearch, SearchMemory};
use self::substitute::LastSubstitute;

use crate::dot::Dot;
use crate::find::Find;
use crate::history::History;
use crate::insert::{Session, Typed};
use crate::keymap::Lookup;
use crate::motion::Destination;
use crate::object::Object;
use crate::register::{Clipboard, Detached, RegisterName, Registers};
use crate::travel::Travel;
use crate::{
    Action, Command, CommandError, Key, Keymap, Mode, Motion, Operator, Prompt, SearchError,
    Substitution, VimError, Wrap,
};

/// Qué pasó con una tecla o una acción.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyOutcome {
    /// Falta el resto de la secuencia o del count.
    Pending,
    Executed(Action),
    /// Texto escrito en modo Insert o en la línea de comandos.
    Inserted,
    /// Ningún binding la usa; la capa de arriba puede probar otro keymap.
    Unbound,
    /// Se envió la línea de comandos con un comando que ejecuta quien aloja
    /// el editor.
    Command(Command),
    /// Se envió la línea de comandos con algo que no es un comando, o con
    /// un `:s` que falló.
    InvalidCommand(CommandError),
    /// Una búsqueda llegó a un borde y siguió por el otro; la UI lo avisa.
    SearchWrapped(Wrap),
    /// Una búsqueda no movió el cursor: no encontró nada o no se pudo hacer.
    SearchFailed(SearchError),
    /// Un `:s` reemplazó algo.
    Substituted(Substitution),
}

/// Estado del modo vim de un editor. No hace I/O: recibe teclas y edita el
/// buffer y las selecciones que le pasan.
#[derive(Debug)]
pub struct Vim {
    keymap: Keymap,
    mode: Mode,
    pending: Vec<Key>,
    count: Option<usize>,
    /// El count tecleado antes de `"x` o del operador; se multiplica con
    /// el de después.
    operator_count: Option<usize>,
    /// El registro elegido con `"x` para lo siguiente.
    selected: Option<RegisterName>,
    registers: Registers,
    /// `"+` y `"*` cuando quien aloja el motor no da un portapapeles.
    detached: Detached,
    command_line: String,
    prompt: Prompt,
    command_history: History,
    /// Tras `d/` o `3/`, el operador y el count esperan al patrón.
    awaiting_search: Option<AwaitingSearch>,
    search: SearchMemory,
    last_substitute: Option<LastSubstitute>,
    /// La pasada por Insert en curso, si se está en Insert.
    insert: Option<Session>,
    /// Lo que repite `.`.
    last_change: Option<Dot>,
    /// Lo tecleado en la última pasada por Insert: el registro `".`.
    last_insert: Option<Vec<Typed>>,
    /// Lo que espera la tecla siguiente como carácter, como el de `f`.
    awaiting: Option<Awaiting>,
    last_find: Option<Find>,
}

impl Default for Vim {
    fn default() -> Self {
        Self::new()
    }
}

impl Vim {
    #[must_use]
    pub fn new() -> Self {
        Self::with_keymap(Keymap::vim())
    }

    #[must_use]
    pub fn with_keymap(keymap: Keymap) -> Self {
        Self {
            keymap,
            mode: Mode::Normal,
            pending: Vec::new(),
            count: None,
            operator_count: None,
            selected: None,
            registers: Registers::default(),
            detached: Detached::default(),
            command_line: String::new(),
            prompt: Prompt::Command,
            command_history: History::default(),
            awaiting_search: None,
            search: SearchMemory::default(),
            last_substitute: None,
            insert: None,
            last_change: None,
            last_insert: None,
            awaiting: None,
            last_find: None,
        }
    }

    #[must_use]
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Como [`Vim::feed_with_clipboard`], con `"+` y `"*` como registros
    /// normales.
    ///
    /// # Errors
    /// Solo si una edición resulta inválida para el buffer, lo que sería un bug.
    pub fn feed(
        &mut self,
        key: Key,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<KeyOutcome, VimError> {
        self.with_detached(|vim, clipboard| {
            vim.feed_with_clipboard(key, buffer, selections, clipboard)
        })
    }

    /// Pasa una tecla al motor; `clipboard` es lo que leen y escriben `"+`
    /// y `"*`.
    ///
    /// # Errors
    /// Solo si una edición resulta inválida para el buffer, lo que sería un bug.
    pub fn feed_with_clipboard(
        &mut self,
        key: Key,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<KeyOutcome, VimError> {
        if let Some(awaiting) = self.awaiting.take() {
            return self.complete(awaiting, key, buffer, selections, clipboard);
        }
        if self.push_count_digit(key) {
            return Ok(KeyOutcome::Pending);
        }
        self.pending.push(key);
        match self.keymap.lookup(self.mode, &self.pending) {
            Lookup::Pending => Ok(KeyOutcome::Pending),
            Lookup::Action(action) => {
                self.pending.clear();
                self.run(action, buffer, selections, clipboard)
            }
            Lookup::Unbound => self.unbound(buffer, selections),
        }
    }

    /// Como [`Vim::execute_with_clipboard`], con `"+` y `"*` como registros
    /// normales.
    ///
    /// # Errors
    /// Solo si una edición resulta inválida para el buffer, lo que sería un bug.
    pub fn execute(
        &mut self,
        action: Action,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<KeyOutcome, VimError> {
        self.with_detached(|vim, clipboard| {
            vim.execute_with_clipboard(action, buffer, selections, clipboard)
        })
    }

    /// Ejecuta una acción por nombre ya resuelto, como harán la paleta y `:`.
    /// Las acciones que no aplican al modo actual solo cancelan lo pendiente.
    /// Enviar la línea de comandos devuelve el comando para quien aloja el
    /// editor.
    ///
    /// # Errors
    /// Solo si una edición resulta inválida para el buffer, lo que sería un bug.
    pub fn execute_with_clipboard(
        &mut self,
        action: Action,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<KeyOutcome, VimError> {
        self.run(action, buffer, selections, clipboard)
    }

    fn with_detached<T>(&mut self, run: impl FnOnce(&mut Self, &mut Detached) -> T) -> T {
        let mut detached = std::mem::take(&mut self.detached);
        let result = run(self, &mut detached);
        self.detached = detached;
        result
    }

    fn run(
        &mut self,
        action: Action,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<KeyOutcome, VimError> {
        self.awaiting = None;
        match (action, self.mode) {
            (Action::Motion(motion), Mode::OperatorPending(operator)) => {
                let object = Object::Motion(motion).into();
                self.operate(operator, object, buffer, selections, clipboard)?;
            }
            (Action::Motion(motion), Mode::Normal) => {
                let count = self.take_count();
                self.reset();
                *selections = move_cursors(selections, |s| motion.resolve(buffer, s, count, None));
            }
            (Action::FindChar(search), Mode::Normal | Mode::OperatorPending(_)) => {
                self.awaiting = Some(Awaiting::Find(search));
            }
            (Action::RepeatFind { reversed }, Mode::Normal | Mode::OperatorPending(_)) => {
                self.repeat_find(reversed, buffer, selections, clipboard)?;
            }
            (Action::Search { backward }, Mode::Normal | Mode::OperatorPending(_)) => {
                self.open_search(backward);
            }
            (Action::SearchNext { reversed }, Mode::Normal | Mode::OperatorPending(_)) => {
                return self.search_next(reversed, buffer, selections, clipboard);
            }
            (
                Action::SearchWord {
                    backward,
                    whole_word,
                },
                Mode::Normal | Mode::OperatorPending(_),
            ) => return self.search_word(backward, whole_word, buffer, selections, clipboard),
            (Action::ClearSearchHighlight, Mode::Normal) => {
                self.reset();
                self.search.highlight = false;
            }
            (Action::TextObject(object), Mode::OperatorPending(operator)) => {
                let object = Object::TextObject(object).into();
                self.operate(operator, object, buffer, selections, clipboard)?;
            }
            (Action::CurrentLine, Mode::OperatorPending(operator)) => {
                let object = Object::CurrentLine.into();
                self.operate(operator, object, buffer, selections, clipboard)?;
            }
            (Action::Operator(operator), Mode::Normal) => {
                self.operator_count = self.take_count();
                self.mode = Mode::OperatorPending(operator);
            }
            (Action::SelectRegister, Mode::Normal) => self.awaiting = Some(Awaiting::Register),
            (Action::DeleteRight, Mode::Normal) => {
                let object = Object::Motion(Motion::Right).into();
                self.operate(Operator::Delete, object, buffer, selections, clipboard)?;
            }
            (Action::Insert(at), Mode::Normal) => self.enter_insert(at, buffer, selections)?,
            (Action::Paste(side), Mode::Normal) => {
                self.paste(side, buffer, selections, clipboard)?;
            }
            (Action::RepeatChange, Mode::Normal) => {
                self.repeat_change(buffer, selections, clipboard)?;
            }
            (Action::Undo, Mode::Normal) => self.travel(Travel::Undo, buffer, selections)?,
            (Action::Redo, Mode::Normal) => self.travel(Travel::Redo, buffer, selections)?,
            (Action::Earlier, Mode::Normal) => self.travel(Travel::Earlier, buffer, selections)?,
            (Action::Later, Mode::Normal) => self.travel(Travel::Later, buffer, selections)?,
            (Action::NormalMode, Mode::Insert) => {
                self.repeat_insert(buffer, selections)?;
                self.leave_insert(buffer, selections)?;
            }
            (Action::Interrupt, Mode::Insert) => self.leave_insert(buffer, selections)?,
            (Action::Backspace, Mode::Insert) => {
                self.type_in_insert(&Typed::Backspace, buffer, selections)?;
            }
            (Action::Newline, Mode::Insert) => {
                self.type_in_insert(&Typed::Text("\n".to_owned()), buffer, selections)?;
            }
            (Action::CommandLine, Mode::Normal) => self.open_command_line(Prompt::Command),
            (Action::NormalMode | Action::Interrupt, Mode::CommandLine) => {
                self.close_command_line();
            }
            (Action::Backspace, Mode::CommandLine) => self.command_line_backspace(),
            (Action::HistoryPrevious, Mode::CommandLine) => self.recall(true),
            (Action::HistoryNext, Mode::CommandLine) => self.recall(false),
            (Action::SubmitCommand, Mode::CommandLine) => {
                return self.submit_command_line(buffer, selections, clipboard);
            }
            _ => self.reset(),
        }
        Ok(KeyOutcome::Executed(action))
    }

    /// El cursor se movió por fuera del motor, como con un clic o un pegado
    /// en Normal: se cancela lo pendiente (count, operador, `f`) y, como en
    /// vim, lo tecleado en Insert ya no se repite con el count y lo que se
    /// teclee después es otro cambio para `.`.
    pub fn cursor_moved(&mut self) {
        self.reset();
        if let Some(session) = &mut self.insert {
            session.cursor_moved();
        }
    }

    /// Un `0` sin count previo es la motion `0`, no un dígito.
    fn push_count_digit(&mut self, key: Key) -> bool {
        let Key::Char(c) = key else { return false };
        let starts_count = c != '0' || self.count.is_some();
        let accepts_count = !self.mode.takes_text() && self.pending.is_empty();
        let Some(digit) = c.to_digit(10).filter(|_| starts_count && accepts_count) else {
            return false;
        };
        let count = self.count.unwrap_or(0);
        self.count = Some(count.saturating_mul(10).saturating_add(digit as usize));
        true
    }

    fn unbound(
        &mut self,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<KeyOutcome, VimError> {
        let keys = std::mem::take(&mut self.pending);
        if !self.mode.takes_text() {
            self.reset();
            return Ok(KeyOutcome::Unbound);
        }
        let text: String = keys
            .iter()
            .filter_map(|&key| typed_char(key, self.mode))
            .collect();
        if text.is_empty() {
            return Ok(KeyOutcome::Unbound);
        }
        self.insert_text(&text, buffer, selections)
    }

    /// El count de lo que se va a ejecutar: `2"a3x` borra 6.
    fn take_count(&mut self) -> Option<usize> {
        combined(self.operator_count.take(), self.count.take())
    }

    fn reset(&mut self) {
        self.pending.clear();
        self.count = None;
        self.operator_count = None;
        self.selected = None;
        self.awaiting = None;
        if matches!(self.mode, Mode::OperatorPending(_)) {
            self.mode = Mode::Normal;
        }
    }
}

/// Cada cursor va a donde lo lleve `resolve`; si falla, se queda.
pub(super) fn move_cursors(
    selections: &Selections,
    resolve: impl Fn(&Selection) -> Option<Destination>,
) -> Selections {
    selections.map(|selection| {
        resolve(selection).map_or(*selection, |destination| {
            Selection::cursor(destination.offset).with_goal(destination.goal)
        })
    })
}

/// `2d3w` opera sobre 6 palabras; sin ningún count, `None`.
fn combined(operator: Option<usize>, motion: Option<usize>) -> Option<usize> {
    match (operator, motion) {
        (None, None) => None,
        (a, b) => Some(a.unwrap_or(1).saturating_mul(b.unwrap_or(1))),
    }
}

fn typed_char(key: Key, mode: Mode) -> Option<char> {
    match key {
        Key::Char(c) => Some(c),
        Key::Tab if mode == Mode::Insert => Some('\t'),
        _ => None,
    }
}
