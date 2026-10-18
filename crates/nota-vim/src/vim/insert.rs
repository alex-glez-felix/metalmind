//! Entrar a Insert, teclear y salir.

use nota_buffer::{Buffer, Selections};

use super::Vim;
use crate::dot::{Change, Dot};
use crate::insert::{self, Session, Typed};
use crate::{InsertAt, Key, KeyOutcome, Mode, VimError};

impl Vim {
    /// Texto que llega entero, como lo pegado: entra por el mismo camino que
    /// lo tecleado, así que en Insert también lo repiten el count y `.`.
    /// Fuera de Insert y de la línea de comandos no hace nada.
    ///
    /// # Errors
    /// Solo si una edición resulta inválida para el buffer, lo que sería un bug.
    pub fn insert_text(
        &mut self,
        text: &str,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<KeyOutcome, VimError> {
        if self.mode.takes_text() {
            self.record(text.chars().map(Key::Char));
        }
        self.write_text(text, buffer, selections)
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

    pub(super) fn enter_insert(
        &mut self,
        at: InsertAt,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        let count = self.count.take();
        self.reset();
        *selections = insert::enter(buffer, selections, at)?;
        self.start_insert(Dot::new(Change::Insert(at), count, None));
        Ok(())
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
        *selections = typed.apply(buffer, selections)?;
        if let Some(session) = &mut self.insert {
            session.record(typed);
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
            *selections = session.replay(buffer, selections)?;
        }
        Ok(())
    }

    /// Sale a Normal; lo hecho en Insert pasa a ser lo que repite `.`.
    pub(super) fn leave_insert(
        &mut self,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        *selections = insert::leave(buffer, selections)?;
        if let Some(session) = self.insert.take() {
            let dot = session.into_dot();
            self.last_insert = Some(dot.typed.clone());
            self.last_change = Some(dot);
        }
        self.mode = Mode::Normal;
        Ok(())
    }
}
