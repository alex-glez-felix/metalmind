use crate::{BufferError, ByteOffset, Selections};

/// Edición ya aplicada, con lo necesario para deshacerla y rehacerla.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AppliedEdit {
    pub start: ByteOffset,
    pub deleted: String,
    pub inserted: String,
}

/// Unidad de undo: todas las ediciones de un comando, en orden de aplicación.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transaction {
    edits: Vec<AppliedEdit>,
    selections_before: Selections,
    selections_after: Selections,
}

impl Transaction {
    #[must_use]
    pub fn selections_before(&self) -> &Selections {
        &self.selections_before
    }

    #[must_use]
    pub fn selections_after(&self) -> &Selections {
        &self.selections_after
    }

    pub(crate) fn edits(&self) -> &[AppliedEdit] {
        &self.edits
    }
}

#[derive(Debug)]
struct OpenTransaction {
    before: Selections,
    edits: Vec<AppliedEdit>,
}

#[derive(Debug, Default)]
pub(crate) struct History {
    undo: Vec<Transaction>,
    redo: Vec<Transaction>,
    open: Option<OpenTransaction>,
}

impl History {
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    pub fn begin(&mut self, before: Selections) -> Result<(), BufferError> {
        if self.is_open() {
            return Err(BufferError::TransactionAlreadyOpen);
        }
        self.open = Some(OpenTransaction {
            before,
            edits: Vec::new(),
        });
        Ok(())
    }

    pub fn ensure_open(&self) -> Result<(), BufferError> {
        if self.is_open() {
            Ok(())
        } else {
            Err(BufferError::NoOpenTransaction)
        }
    }

    pub fn record(
        &mut self,
        edits: impl IntoIterator<Item = AppliedEdit>,
    ) -> Result<(), BufferError> {
        let open = self.open.as_mut().ok_or(BufferError::NoOpenTransaction)?;
        open.edits.extend(edits);
        Ok(())
    }

    /// Una transacción sin ediciones no deja rastro en el historial.
    pub fn commit(&mut self, after: Selections) -> Result<(), BufferError> {
        let open = self.open.take().ok_or(BufferError::NoOpenTransaction)?;
        if open.edits.is_empty() {
            return Ok(());
        }
        self.redo.clear();
        self.undo.push(Transaction {
            edits: open.edits,
            selections_before: open.before,
            selections_after: after,
        });
        Ok(())
    }

    pub fn take_undo(&mut self) -> Result<Option<Transaction>, BufferError> {
        self.ensure_closed()?;
        Ok(self.undo.pop())
    }

    pub fn take_redo(&mut self) -> Result<Option<Transaction>, BufferError> {
        self.ensure_closed()?;
        Ok(self.redo.pop())
    }

    pub fn push_undone(&mut self, transaction: Transaction) -> Option<&Transaction> {
        self.redo.push(transaction);
        self.redo.last()
    }

    pub fn push_redone(&mut self, transaction: Transaction) -> Option<&Transaction> {
        self.undo.push(transaction);
        self.undo.last()
    }

    fn ensure_closed(&self) -> Result<(), BufferError> {
        if self.is_open() {
            Err(BufferError::TransactionAlreadyOpen)
        } else {
            Ok(())
        }
    }
}
