//! Recorrer el historial: undo, redo, `g-` y `g+`.

use super::{Buffer, locate, splice};
use crate::history::{Direction, Step};
use crate::{BufferError, Transaction};

impl Buffer {
    /// Vuelve al estado anterior en la rama actual y devuelve la transacción
    /// deshecha.
    ///
    /// # Errors
    /// [`BufferError::TransactionAlreadyOpen`] si hay una transacción abierta.
    pub fn undo(&mut self) -> Result<Option<&Transaction>, BufferError> {
        let steps = self.history.undo()?;
        self.travel(&steps)
    }

    /// Rehace la transacción deshecha o visitada más recientemente desde el
    /// estado actual y la devuelve.
    ///
    /// # Errors
    /// [`BufferError::TransactionAlreadyOpen`] si hay una transacción abierta.
    pub fn redo(&mut self) -> Result<Option<&Transaction>, BufferError> {
        let steps = self.history.redo()?;
        self.travel(&steps)
    }

    /// Retrocede `steps` estados en orden cronológico, aunque estén en otra
    /// rama, como `g-` en vim. Devuelve la última transacción recorrida.
    ///
    /// # Errors
    /// [`BufferError::TransactionAlreadyOpen`] si hay una transacción abierta.
    pub fn earlier(&mut self, steps: usize) -> Result<Option<&Transaction>, BufferError> {
        let steps = self.history.earlier(steps)?;
        self.travel(&steps)
    }

    /// Avanza `steps` estados en orden cronológico, aunque estén en otra
    /// rama, como `g+` en vim. Devuelve la última transacción recorrida.
    ///
    /// # Errors
    /// [`BufferError::TransactionAlreadyOpen`] si hay una transacción abierta.
    pub fn later(&mut self, steps: usize) -> Result<Option<&Transaction>, BufferError> {
        let steps = self.history.later(steps)?;
        self.travel(&steps)
    }

    /// Deshace o rehace cada transacción del recorrido y devuelve la última.
    /// Como en vim, cada una deja las anclas donde estaban del otro lado.
    fn travel(&mut self, steps: &[Step]) -> Result<Option<&Transaction>, BufferError> {
        for step in steps {
            let Some(transaction) = self.history.transaction(step.seq) else {
                continue;
            };
            let anchors_before = self.anchors.clone();
            let edits = transaction.edits();
            match step.direction {
                Direction::Undo => {
                    for edit in edits.iter().rev() {
                        let spots = locate(&self.rope, edit.start, edit.inserted.len())?;
                        splice(
                            &mut self.rope,
                            &mut self.anchors,
                            &mut self.marked,
                            &mut self.changes,
                            spots,
                            &edit.deleted,
                            None,
                        )?;
                    }
                }
                Direction::Redo => {
                    for edit in edits {
                        let spots = locate(&self.rope, edit.start, edit.deleted.len())?;
                        splice(
                            &mut self.rope,
                            &mut self.anchors,
                            &mut self.marked,
                            &mut self.changes,
                            spots,
                            &edit.inserted,
                            None,
                        )?;
                    }
                }
            }
            self.version += 1;
            let saved = self.history.swap_anchors(step.seq, anchors_before);
            self.anchors.restore(&saved);
        }
        Ok(steps
            .last()
            .and_then(|step| self.history.transaction(step.seq)))
    }
}
