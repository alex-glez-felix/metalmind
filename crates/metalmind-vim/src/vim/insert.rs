//! Entrar a Insert, teclear y salir.

use metalmind_buffer::{Buffer, Selections};

use super::Vim;
use crate::dot::{Change, Dot};
use crate::insert::{self, Session, Typed, Typing};
use crate::{InsertAt, Key, KeyOutcome, Mode, VimError};

impl Vim {
    /// Texto que llega entero, como lo pegado. En Insert y en la línea de
    /// comandos entra por el mismo camino que lo tecleado, así que en Insert
    /// también lo repiten el count y `.`. Si no, como un clic cancela lo
    /// pendiente y sale de Visual, y lo pegado es un paso propio de undo con
    /// el cursor sobre su último carácter.
    ///
    /// # Errors
    /// Solo si una edición resulta inválida para el buffer, lo que sería un bug.
    pub fn paste_text(
        &mut self,
        text: &str,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<KeyOutcome, VimError> {
        let version = buffer.version();
        let outcome = if self.mode.takes_text() {
            self.record(text.chars().map(Key::Char));
            self.write_text(text, buffer, selections)?
        } else {
            self.cursor_moved();
            *selections = insert::paste_at_cursor(buffer, selections, text)?;
            KeyOutcome::Inserted
        };
        self.follow_cursor(buffer, selections, version);
        Ok(outcome)
    }

    /// Escribe en Insert o en la línea de comandos, sin grabarlo en la macro.
    pub(super) fn write_text(
        &mut self,
        text: &str,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<KeyOutcome, VimError> {
        match self.mode {
            Mode::Insert => {
                self.type_in_insert(&Typed::Text(text.to_owned()), buffer, selections)?;
            }
            Mode::CommandLine => self.type_in_command_line(text),
            Mode::Normal | Mode::OperatorPending(_) | Mode::Visual(_) => {
                return Ok(KeyOutcome::Unbound);
            }
        }
        Ok(KeyOutcome::Inserted)
    }

    /// `list` es si `o` y `O` pueden abrir un ítem de lista: `.` no lo hace
    /// si el cambio que repite no lo hizo.
    pub(super) fn enter_insert(
        &mut self,
        at: InsertAt,
        list: bool,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        let count = self.count.take();
        self.reset();
        let typing = self.typing().lists_if(list);
        let (entered, list) = insert::enter(buffer, selections, at, typing, self.layout.as_mut())?;
        *selections = entered;
        self.start_insert(Dot::new(Change::Insert { at, list }, count, None));
        Ok(())
    }

    pub(super) fn typing(&self) -> Typing {
        Typing {
            continue_lists: self.markdown.continue_lists,
            columns: self.viewport.columns(),
        }
    }

    /// Pasa a Insert con la transacción ya abierta por quien empezó `dot`.
    pub(super) fn start_insert(&mut self, dot: Dot) {
        self.insert = Some(Session::new(dot));
        self.mode = Mode::Insert;
    }

    pub(super) fn type_in_insert(
        &mut self,
        typed: &Typed,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        let typing = self.typing();
        let (typed_now, done) = typed.apply(buffer, selections, typing, self.layout.as_mut())?;
        *selections = typed_now;
        if let Some(session) = &mut self.insert {
            session.record(&done);
        }
        Ok(())
    }

    /// Lo que pide el count de `3ihola<Esc>`; `ctrl-c` sale sin esto.
    pub(super) fn repeat_insert(
        &mut self,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        if let Some(session) = &self.insert {
            *selections = session.replay(buffer, selections, self.typing(), &mut self.layout)?;
        }
        Ok(())
    }

    /// `<Esc>`: lo que pide el count, lo que un bloque repite en sus demás
    /// líneas y salir.
    pub(super) fn finish_insert(
        &mut self,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        self.repeat_insert(buffer, selections)?;
        let corners = self.replicate_in_block(buffer, selections)?;
        self.leave_insert_to(corners, buffer, selections)
    }

    /// Sale a Normal; lo hecho en Insert pasa a ser lo que repite `.`.
    pub(super) fn leave_insert(
        &mut self,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        self.leave_insert_to(None, buffer, selections)
    }

    /// Cierra la transacción con el cursor en `after` o, si no, sobre el
    /// último carácter escrito.
    fn leave_insert_to(
        &mut self,
        after: Option<Selections>,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        self.block_insert = None;
        *selections = after.unwrap_or_else(|| insert::left_of_cursors(buffer, selections));
        buffer.commit_transaction(selections.clone())?;
        if let Some(session) = self.insert.take() {
            let dot = session.into_dot();
            self.last_insert = Some(dot.typed.clone());
            self.last_change = Some(dot);
        }
        self.mode = Mode::Normal;
        Ok(())
    }
}
