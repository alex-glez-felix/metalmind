mod find;

use nota_buffer::{Buffer, Selection, Selections};

use crate::find::Find;
use crate::insert::{Repeat, Typed};
use crate::keymap::Lookup;
use crate::motion::Destination;
use crate::object::Object;
use crate::operator::Operand;
use crate::register::Register;
use crate::travel::Travel;
use crate::{
    Action, CharSearch, Command, CommandError, InsertAt, Key, Keymap, Mode, Motion, Operator,
    PasteSide, VimError, insert, paste,
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
    /// Se envió la línea de comandos con algo que no es un comando.
    InvalidCommand(CommandError),
}

/// Estado del modo vim de un editor. No hace I/O: recibe teclas y edita el
/// buffer y las selecciones que le pasan.
#[derive(Debug)]
pub struct Vim {
    keymap: Keymap,
    mode: Mode,
    pending: Vec<Key>,
    count: Option<usize>,
    operator_count: Option<usize>,
    register: Option<Register>,
    command_line: String,
    repeat: Repeat,
    /// Tras `f`, `F`, `t` o `T`, la tecla siguiente es el carácter a buscar.
    awaiting_char: Option<CharSearch>,
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
            register: None,
            command_line: String::new(),
            repeat: Repeat::default(),
            awaiting_char: None,
            last_find: None,
        }
    }

    #[must_use]
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Lo escrito tras `:`, mientras se está en la línea de comandos.
    #[must_use]
    pub fn command_line(&self) -> Option<&str> {
        (self.mode == Mode::CommandLine).then_some(self.command_line.as_str())
    }

    /// # Errors
    /// Solo si una edición resulta inválida para el buffer, lo que sería un bug.
    pub fn feed(
        &mut self,
        key: Key,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<KeyOutcome, VimError> {
        if let Some(search) = self.awaiting_char.take() {
            return self.find_char(search, key, buffer, selections);
        }
        if self.push_count_digit(key) {
            return Ok(KeyOutcome::Pending);
        }
        self.pending.push(key);
        match self.keymap.lookup(self.mode, &self.pending) {
            Lookup::Pending => Ok(KeyOutcome::Pending),
            Lookup::Action(action) => {
                self.pending.clear();
                self.execute(action, buffer, selections)
            }
            Lookup::Unbound => self.unbound(buffer, selections),
        }
    }

    /// Ejecuta una acción por nombre ya resuelto, como harán la paleta y `:`.
    /// Las acciones que no aplican al modo actual solo cancelan lo pendiente.
    /// Enviar la línea de comandos devuelve el comando para quien aloja el
    /// editor.
    ///
    /// # Errors
    /// Solo si una edición resulta inválida para el buffer, lo que sería un bug.
    pub fn execute(
        &mut self,
        action: Action,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<KeyOutcome, VimError> {
        self.awaiting_char = None;
        match (action, self.mode) {
            (Action::Motion(motion), Mode::OperatorPending(operator)) => {
                self.operate(operator, Object::Motion(motion), buffer, selections)?;
            }
            (Action::Motion(motion), Mode::Normal) => {
                let count = self.count.take();
                *selections = move_cursors(selections, |s| motion.resolve(buffer, s, count, None));
            }
            (Action::FindChar(search), Mode::Normal | Mode::OperatorPending(_)) => {
                self.awaiting_char = Some(search);
            }
            (Action::RepeatFind { reversed }, Mode::Normal | Mode::OperatorPending(_)) => {
                self.repeat_find(reversed, buffer, selections)?;
            }
            (Action::TextObject(object), Mode::OperatorPending(operator)) => {
                self.operate(operator, Object::TextObject(object), buffer, selections)?;
            }
            (Action::CurrentLine, Mode::OperatorPending(operator)) => {
                self.operate(operator, Object::CurrentLine, buffer, selections)?;
            }
            (Action::Operator(operator), Mode::Normal) => {
                self.operator_count = self.count.take();
                self.mode = Mode::OperatorPending(operator);
            }
            (Action::DeleteRight, Mode::Normal) => {
                self.operate(
                    Operator::Delete,
                    Object::Motion(Motion::Right),
                    buffer,
                    selections,
                )?;
            }
            (Action::Insert(at), Mode::Normal) => self.enter_insert(at, buffer, selections)?,
            (Action::Paste(side), Mode::Normal) => self.paste(side, buffer, selections)?,
            (Action::Undo, Mode::Normal) => self.travel(Travel::Undo, buffer, selections)?,
            (Action::Redo, Mode::Normal) => self.travel(Travel::Redo, buffer, selections)?,
            (Action::Earlier, Mode::Normal) => self.travel(Travel::Earlier, buffer, selections)?,
            (Action::Later, Mode::Normal) => self.travel(Travel::Later, buffer, selections)?,
            (Action::NormalMode, Mode::Insert) => {
                *selections = self.repeat.replay(buffer, selections)?;
                self.leave_insert(buffer, selections)?;
            }
            (Action::Interrupt, Mode::Insert) => self.leave_insert(buffer, selections)?,
            (Action::Backspace, Mode::Insert) => {
                self.type_in_insert(&Typed::Backspace, buffer, selections)?;
            }
            (Action::Newline, Mode::Insert) => {
                self.type_in_insert(&Typed::Text("\n".to_owned()), buffer, selections)?;
            }
            (Action::CommandLine, Mode::Normal) => {
                self.reset();
                self.command_line.clear();
                self.mode = Mode::CommandLine;
            }
            (Action::NormalMode | Action::Interrupt, Mode::CommandLine) => {
                self.mode = Mode::Normal;
            }
            (Action::Backspace, Mode::CommandLine) => {
                if self.command_line.pop().is_none() {
                    self.mode = Mode::Normal;
                }
            }
            (Action::SubmitCommand, Mode::CommandLine) => return Ok(self.submit_command()),
            _ => self.reset(),
        }
        Ok(KeyOutcome::Executed(action))
    }

    /// El cursor se movió por fuera del motor, como con un clic o un pegado
    /// en Normal: se cancela lo pendiente (count, operador, `f`) y, como en
    /// vim, lo tecleado en Insert ya no se repite con el count.
    pub fn cursor_moved(&mut self) {
        self.reset();
        self.repeat = Repeat::default();
    }

    /// Texto que llega entero, como lo pegado: entra por el mismo camino que
    /// lo tecleado, así que en Insert también se repite con el count. Fuera
    /// de Insert y de la línea de comandos no hace nada.
    ///
    /// # Errors
    /// Solo si una edición resulta inválida para el buffer, lo que sería un bug.
    pub fn insert_text(
        &mut self,
        text: &str,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<KeyOutcome, VimError> {
        match self.mode {
            Mode::Insert => {
                self.type_in_insert(&Typed::Text(text.to_owned()), buffer, selections)?;
            }
            Mode::CommandLine => self.command_line.push_str(text),
            Mode::Normal | Mode::OperatorPending(_) => return Ok(KeyOutcome::Unbound),
        }
        Ok(KeyOutcome::Inserted)
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

    fn submit_command(&mut self) -> KeyOutcome {
        self.mode = Mode::Normal;
        match Command::parse(&std::mem::take(&mut self.command_line)) {
            Ok(Some(command)) => KeyOutcome::Command(command),
            Ok(None) => KeyOutcome::Executed(Action::SubmitCommand),
            Err(error) => KeyOutcome::InvalidCommand(error),
        }
    }

    fn reset(&mut self) {
        self.pending.clear();
        self.count = None;
        self.operator_count = None;
        self.awaiting_char = None;
        if matches!(self.mode, Mode::OperatorPending(_)) {
            self.mode = Mode::Normal;
        }
    }

    fn operate(
        &mut self,
        operator: Operator,
        object: Object,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        let count = combined(self.operator_count.take(), self.count.take());
        self.reset();
        let operands = selections.map_to(|selection| Operand {
            selection: *selection,
            target: object.target(buffer, selection, operator, count),
        });
        let Some(outcome) = operator.apply(buffer, &operands)? else {
            return Ok(());
        };
        *selections = outcome.selections;
        if outcome.register.is_some() {
            self.register = outcome.register;
        }
        if operator == Operator::Change {
            self.mode = Mode::Insert;
        }
        Ok(())
    }

    fn enter_insert(
        &mut self,
        at: InsertAt,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        let count = self.count.take();
        self.reset();
        *selections = insert::enter(buffer, selections, at)?;
        self.repeat = Repeat::new(count, at);
        self.mode = Mode::Insert;
        Ok(())
    }

    fn type_in_insert(
        &mut self,
        typed: &Typed,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        *selections = typed.apply(buffer, selections)?;
        self.repeat.record(typed);
        Ok(())
    }

    fn leave_insert(
        &mut self,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        *selections = insert::leave(buffer, selections)?;
        self.repeat = Repeat::default();
        self.mode = Mode::Normal;
        Ok(())
    }

    fn paste(
        &mut self,
        side: PasteSide,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        let count = self.count.take().unwrap_or(1);
        self.reset();
        if let Some(register) = &self.register {
            *selections = paste::paste(buffer, selections, register, side, count)?;
        }
        Ok(())
    }

    fn travel(
        &mut self,
        travel: Travel,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        let times = self.count.take().unwrap_or(1);
        self.reset();
        if let Some(restored) = travel.run(buffer, times)? {
            *selections = restored;
        }
        Ok(())
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
