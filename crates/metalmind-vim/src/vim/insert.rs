//! Entrar a Insert, teclear y salir.

use metalmind_buffer::Selections;

use super::Vim;
use crate::Target;
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
    pub fn paste_text(&mut self, text: &str, t: &mut Target<'_>) -> Result<KeyOutcome, VimError> {
        let version = t.buffer.version();
        let outcome = if self.mode.takes_text() {
            self.record(text.chars().map(Key::Char));
            self.write_text(text, t)?
        } else {
            self.cursor_moved();
            *t.selections = insert::paste_at_cursor(t.buffer, t.selections, text)?;
            KeyOutcome::Inserted
        };
        self.follow_cursor(t, version);
        Ok(outcome)
    }

    /// Escribe en Insert o en la línea de comandos, sin grabarlo en la macro.
    pub(super) fn write_text(
        &mut self,
        text: &str,
        t: &mut Target<'_>,
    ) -> Result<KeyOutcome, VimError> {
        match self.mode {
            Mode::Insert => {
                self.type_in_insert(&Typed::Text(text.to_owned()), t)?;
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
        t: &mut Target<'_>,
    ) -> Result<(), VimError> {
        let count = self.count.take();
        self.reset();
        let typing = self.typing(t).lists_if(list);
        let (entered, list) = insert::enter(t.buffer, t.selections, at, typing, t.doc)?;
        *t.selections = entered;
        self.start_insert(Dot::new(Change::Insert { at, list }, count, None));
        Ok(())
    }

    /// Cómo se escribe en el panel de `t`.
    pub(super) fn typing(&self, t: &Target<'_>) -> Typing {
        Typing {
            continue_lists: self.markdown.continue_lists,
            columns: t.pane.viewport.columns(),
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
        t: &mut Target<'_>,
    ) -> Result<(), VimError> {
        let typing = self.typing(t);
        let (typed_now, done) = typed.apply(t.buffer, t.selections, typing, t.doc)?;
        *t.selections = typed_now;
        if let Some(session) = &mut self.insert {
            session.record(&done);
        }
        Ok(())
    }

    /// Lo que pide el count de `3ihola<Esc>`; `ctrl-c` sale sin esto.
    pub(super) fn repeat_insert(&mut self, t: &mut Target<'_>) -> Result<(), VimError> {
        let typing = self.typing(t);
        if let Some(session) = &self.insert {
            *t.selections = session.replay(t.buffer, t.selections, typing, t.doc)?;
        }
        Ok(())
    }

    /// `<Esc>`: lo que pide el count, lo que un bloque repite en sus demás
    /// líneas y salir.
    pub(super) fn finish_insert(&mut self, t: &mut Target<'_>) -> Result<(), VimError> {
        self.repeat_insert(t)?;
        let corners = self.replicate_in_block(t)?;
        self.leave_insert_to(corners, t)
    }

    /// Sale a Normal; lo hecho en Insert pasa a ser lo que repite `.`.
    pub(super) fn leave_insert(&mut self, t: &mut Target<'_>) -> Result<(), VimError> {
        self.leave_insert_to(None, t)
    }

    /// Cierra la transacción con el cursor en `after` o, si no, sobre el
    /// último carácter escrito.
    fn leave_insert_to(
        &mut self,
        after: Option<Selections>,
        t: &mut Target<'_>,
    ) -> Result<(), VimError> {
        self.block_insert = None;
        *t.selections = after.unwrap_or_else(|| insert::left_of_cursors(t.buffer, t.selections));
        t.buffer.commit_transaction(t.selections.clone())?;
        if let Some(session) = self.insert.take() {
            let dot = session.into_dot();
            self.last_insert = Some(dot.typed.clone());
            self.last_change = Some(dot);
        }
        self.mode = Mode::Normal;
        Ok(())
    }
}
