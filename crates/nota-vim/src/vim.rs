use nota_buffer::{Buffer, BufferError, Selection, Selections, Transaction};

use crate::keymap::Lookup;
use crate::lines::clamp_normal;
use crate::operator::Operand;
use crate::register::Register;
use crate::span::{Region, Target};
use crate::{
    Action, InsertAt, Key, Keymap, Motion, Operator, PasteSide, TextObject, VimError, insert, paste,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    OperatorPending(Operator),
}

/// Qué pasó con una tecla.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyOutcome {
    /// Falta el resto de la secuencia o del count.
    Pending,
    Executed(Action),
    /// Texto escrito en modo Insert.
    Inserted,
    /// Ningún binding la usa; la capa de arriba puede probar otro keymap.
    Unbound,
}

/// Sobre qué actúa un operador.
#[derive(Clone, Copy)]
enum Object {
    Motion(Motion),
    TextObject(TextObject),
    CurrentLine,
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
        }
    }

    #[must_use]
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// # Errors
    /// Solo si una edición resulta inválida para el buffer, lo que sería un bug.
    pub fn feed(
        &mut self,
        key: Key,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<KeyOutcome, VimError> {
        if self.push_count_digit(key) {
            return Ok(KeyOutcome::Pending);
        }
        self.pending.push(key);
        match self.keymap.lookup(self.mode, &self.pending) {
            Lookup::Pending => Ok(KeyOutcome::Pending),
            Lookup::Action(action) => {
                self.pending.clear();
                self.execute(action, buffer, selections)?;
                Ok(KeyOutcome::Executed(action))
            }
            Lookup::Unbound => self.unbound(buffer, selections),
        }
    }

    /// Ejecuta una acción por nombre ya resuelto, como harán la paleta y `:`.
    /// Las acciones que no aplican al modo actual solo cancelan lo pendiente.
    ///
    /// # Errors
    /// Solo si una edición resulta inválida para el buffer, lo que sería un bug.
    pub fn execute(
        &mut self,
        action: Action,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        match (action, self.mode) {
            (Action::Motion(motion), Mode::OperatorPending(operator)) => {
                self.operate(operator, Object::Motion(motion), buffer, selections)?;
            }
            (Action::Motion(motion), Mode::Normal) => {
                let count = self.count.take();
                *selections = move_cursors(buffer, selections, motion, count);
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
                *selections = insert::leave(buffer, selections)?;
                self.mode = Mode::Normal;
            }
            (Action::Backspace, Mode::Insert) => {
                *selections = insert::backspace(buffer, selections)?;
            }
            (Action::Newline, Mode::Insert) => {
                *selections = insert::type_text(buffer, selections, "\n")?;
            }
            _ => self.reset(),
        }
        Ok(())
    }

    /// Un `0` sin count previo es la motion `0`, no un dígito.
    fn push_count_digit(&mut self, key: Key) -> bool {
        let Key::Char(c) = key else { return false };
        let starts_count = c != '0' || self.count.is_some();
        let accepts_count = self.mode != Mode::Insert && self.pending.is_empty();
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
        if self.mode != Mode::Insert {
            self.reset();
            return Ok(KeyOutcome::Unbound);
        }
        let text: String = keys.iter().filter_map(|key| typed_char(*key)).collect();
        if text.is_empty() {
            return Ok(KeyOutcome::Unbound);
        }
        *selections = insert::type_text(buffer, selections, &text)?;
        Ok(KeyOutcome::Inserted)
    }

    fn reset(&mut self) {
        self.pending.clear();
        self.count = None;
        self.operator_count = None;
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
            target: target(buffer, selection, object, operator, count),
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
        self.reset();
        *selections = insert::enter(buffer, selections, at)?;
        self.mode = Mode::Insert;
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

    /// Como vim, moverse por el historial deja el cursor donde empezó el
    /// último cambio recorrido.
    fn travel(
        &mut self,
        travel: Travel,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        let times = self.count.take().unwrap_or(1);
        self.reset();
        let restored = match travel {
            Travel::Undo => repeat(buffer, times, Buffer::undo)?,
            Travel::Redo => repeat(buffer, times, Buffer::redo)?,
            Travel::Earlier => buffer.earlier(times)?.map(cursors_before),
            Travel::Later => buffer.later(times)?.map(cursors_before),
        };
        if let Some(restored) = restored {
            *selections =
                restored.map(|selection| Selection::cursor(clamp_normal(buffer, selection.head)));
        }
        Ok(())
    }
}

/// Cómo se mueve el cursor por el historial: `u`, `ctrl-r`, `g-`, `g+`.
#[derive(Clone, Copy)]
enum Travel {
    Undo,
    Redo,
    Earlier,
    Later,
}

/// Da `times` pasos con `step` mientras haya historial; devuelve las
/// selecciones de antes del último cambio recorrido.
fn repeat(
    buffer: &mut Buffer,
    times: usize,
    step: fn(&mut Buffer) -> Result<Option<&Transaction>, BufferError>,
) -> Result<Option<Selections>, BufferError> {
    let mut restored = None;
    for _ in 0..times {
        let Some(transaction) = step(buffer)? else {
            break;
        };
        restored = Some(cursors_before(transaction));
    }
    Ok(restored)
}

fn cursors_before(transaction: &Transaction) -> Selections {
    transaction.selections_before().clone()
}

fn move_cursors(
    buffer: &Buffer,
    selections: &Selections,
    motion: Motion,
    count: Option<usize>,
) -> Selections {
    selections.map(|selection| {
        motion
            .resolve(buffer, selection, count, None)
            .map_or(*selection, |destination| {
                Selection::cursor(destination.offset).with_goal(destination.goal)
            })
    })
}

fn target(
    buffer: &Buffer,
    selection: &Selection,
    object: Object,
    operator: Operator,
    count: Option<usize>,
) -> Option<Target> {
    let span = match object {
        Object::Motion(motion) => motion
            .resolve(buffer, selection, count, Some(operator))?
            .span_from(selection.head),
        Object::TextObject(object) => object.resolve(buffer, selection.head, count.unwrap_or(1))?,
        Object::CurrentLine => return current_lines(buffer, selection, count.unwrap_or(1)),
    };
    Some(span.into_target(buffer))
}

/// `3dd` falla en la última línea, pero si hay menos de 3 toma las que haya.
fn current_lines(buffer: &Buffer, selection: &Selection, count: usize) -> Option<Target> {
    let first = buffer.line_of(selection.head);
    let last_line = buffer.last_line();
    if count > 1 && first >= last_line {
        return None;
    }
    Some(Target {
        region: Region::Lines {
            first,
            last: first.saturating_add(count - 1).min(last_line),
        },
        start: selection.head,
        adjusted: false,
    })
}

/// `2d3w` opera sobre 6 palabras; sin ningún count, `None`.
fn combined(operator: Option<usize>, motion: Option<usize>) -> Option<usize> {
    match (operator, motion) {
        (None, None) => None,
        (a, b) => Some(a.unwrap_or(1).saturating_mul(b.unwrap_or(1))),
    }
}

fn typed_char(key: Key) -> Option<char> {
    match key {
        Key::Char(c) => Some(c),
        Key::Tab => Some('\t'),
        _ => None,
    }
}
