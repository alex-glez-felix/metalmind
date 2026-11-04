//! La pizarra de un panel, editada desde su vista: lo que toca el archivo pasa
//! por [`Document`]; aquí, lo que además es del motor y del autoguardado.

use metalmind_board::{Board, Frame};
use metalmind_buffer::{Selection, Selections};
use metalmind_vim::{Action, InsertAt};

use super::{DocumentId, Message, PaneId, Workspace, WorkspaceError};

impl Workspace {
    fn board_document(&self, pane: PaneId) -> Result<DocumentId, WorkspaceError> {
        Ok(self.pane(pane)?.document)
    }

    /// Cambia la pizarra de `pane` como un paso del historial.
    ///
    /// # Errors
    /// Si el archivo no se lee como pizarra: no se toca nada.
    pub fn edit_board<T>(
        &mut self,
        pane: PaneId,
        change: impl FnOnce(&mut Board) -> T,
    ) -> Result<T, WorkspaceError> {
        let id = self.board_document(pane)?;
        let result = self.document_mut(id)?.edit_board(change)?;
        self.edited(id);
        Ok(result)
    }

    /// `u`: como vim, avisa si no hay nada que deshacer.
    ///
    /// # Errors
    /// Solo si el estado está inconsistente, lo que sería un bug.
    pub fn undo_board(&mut self, pane: PaneId) -> Result<(), WorkspaceError> {
        self.close_board_text(pane)?;
        let id = self.board_document(pane)?;
        if self
            .document_mut(id)?
            .undo_board()
            .map_err(WorkspaceError::Buffer)?
        {
            self.edited(id);
        } else {
            self.show_message(Message::Info("Ya es el cambio más antiguo".to_owned()));
        }
        Ok(())
    }

    /// `ctrl-r`.
    ///
    /// # Errors
    /// Solo si el estado está inconsistente, lo que sería un bug.
    pub fn redo_board(&mut self, pane: PaneId) -> Result<(), WorkspaceError> {
        self.close_board_text(pane)?;
        let id = self.board_document(pane)?;
        if self
            .document_mut(id)?
            .redo_board()
            .map_err(WorkspaceError::Buffer)?
        {
            self.edited(id);
        } else {
            self.show_message(Message::Info("Ya es el cambio más reciente".to_owned()));
        }
        Ok(())
    }

    /// Abre el texto de `element` en el motor, en Insert al final (09 §4.2);
    /// `false` si no tiene texto.
    ///
    /// # Errors
    /// Si el archivo no se lee como pizarra.
    pub fn open_board_text(&mut self, pane: PaneId, element: &str) -> Result<bool, WorkspaceError> {
        self.close_board_text(pane)?;
        let id = self.board_document(pane)?;
        if !self.document_mut(id)?.begin_board_text(element)? {
            return Ok(false);
        }
        self.start_typing(pane)?;
        Ok(true)
    }

    /// Crea un texto suelto en `frame` y lo abre. Da su id.
    ///
    /// # Errors
    /// Si el archivo no se lee como pizarra.
    pub fn open_new_board_text(
        &mut self,
        pane: PaneId,
        frame: Frame,
    ) -> Result<String, WorkspaceError> {
        self.close_board_text(pane)?;
        let id = self.board_document(pane)?;
        let element = self.document_mut(id)?.begin_new_board_text(frame)?;
        self.edited(id);
        self.start_typing(pane)?;
        Ok(element)
    }

    /// Al final del buffer recién abierto, escribiendo.
    fn start_typing(&mut self, pane: PaneId) -> Result<(), WorkspaceError> {
        self.edit_pane(pane, None, |vim, target| {
            let last_line = target.buffer.last_line();
            let start = target.buffer.line_start(last_line);
            *target.selections = Selections::single(Selection::cursor(start));
            vim.execute(Action::Insert(InsertAt::LineEnd), target)?;
            Ok(())
        })
    }

    /// Cierra la caja abierta en `pane`, si hay una: lo que estuviera a medias
    /// en el motor termina y todo lo escrito queda como un solo paso.
    ///
    /// # Errors
    /// Solo si el estado está inconsistente, lo que sería un bug.
    pub fn close_board_text(&mut self, pane: PaneId) -> Result<(), WorkspaceError> {
        let id = self.board_document(pane)?;
        if self.document(id)?.open_board_text().is_none() {
            return Ok(());
        }
        self.edit_pane(pane, None, |vim, target| {
            vim.execute(Action::Interrupt, target)?;
            Ok(())
        })?;
        self.document_mut(id)?.end_board_text()?;
        self.rewind_cursors_on_board(id);
        self.edited(id);
        if self.document(id)?.changed_on_disk() {
            self.refresh(id)?;
        }
        Ok(())
    }

    /// Lo que se teclea ahora no completa ningún atajo a medias.
    #[must_use]
    pub fn engine_is_idle(&self) -> bool {
        self.vim.pending_keys().is_empty()
    }
}

#[cfg(test)]
#[path = "board_tests.rs"]
mod tests;
