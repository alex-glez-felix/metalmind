//! La ventana de cada panel: cuántas filas le caben y la rueda del ratón.

use metalmind_buffer::Buffer;
use metalmind_vim::Mode;

use super::{Pane, PaneId, Workspace, WorkspaceError};

impl Workspace {
    /// Las filas que le caben a `pane` en este layout. Si cambian, su
    /// cursor se vuelve a mostrar, como en vim al redimensionar.
    ///
    /// # Errors
    /// Si `pane` no existe o su documento tampoco, lo que sería un bug.
    pub fn resize(&mut self, pane: PaneId, rows: usize) -> Result<(), WorkspaceError> {
        let scrolloff = self.scrolloff;
        let (state, buffer) = self.shown(pane)?;
        state.viewport.resize(rows);
        let head = state.selections.primary().head;
        state.viewport.reveal(buffer, head, scrolloff);
        Ok(())
    }

    /// La rueda llevó `pane` hasta `top`; su cursor la sigue, como en vim,
    /// sin darle el foco. Un panel sin foco está en Normal.
    ///
    /// # Errors
    /// Si `pane` no existe o su documento tampoco, lo que sería un bug.
    pub fn scroll_to(&mut self, pane: PaneId, top: usize) -> Result<(), WorkspaceError> {
        if pane == self.focused() {
            return self.edit_pane(pane, |vim, buffer, selections| {
                vim.scroll_to(top, buffer, selections);
                Ok(())
            });
        }
        let scrolloff = self.scrolloff;
        let (state, buffer) = self.shown(pane)?;
        state
            .viewport
            .scroll_to(top, buffer, &mut state.selections, scrolloff, Mode::Normal);
        Ok(())
    }

    /// `pane`, para cambiarlo, y el buffer que muestra.
    fn shown(&mut self, pane: PaneId) -> Result<(&mut Pane, &Buffer), WorkspaceError> {
        let state = self
            .panes
            .get_mut(&pane)
            .ok_or(WorkspaceError::NoPane(pane))?;
        let id = state.document;
        let document = self
            .documents
            .get(id)
            .ok_or(WorkspaceError::NoDocument(id))?;
        Ok((state, document.buffer()))
    }
}

#[cfg(test)]
#[path = "scrolling_tests.rs"]
mod tests;
