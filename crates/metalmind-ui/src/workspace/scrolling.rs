//! La ventana de cada panel: cuántas filas le caben y la rueda del ratón.

use metalmind_syntax::Rows;
use metalmind_vim::Mode;

use super::{Pane, PaneId, Workspace, WorkspaceError};
use crate::document::Editable;

impl Workspace {
    /// El alto de `pane` en este layout y lo que mide cada línea, en
    /// píxeles. Si cambia, su cursor se vuelve a mostrar, como en vim al
    /// redimensionar. Mide las líneas que se van a pintar.
    ///
    /// # Errors
    /// Si `pane` no existe o su documento tampoco, lo que sería un bug.
    pub fn resize(
        &mut self,
        pane: PaneId,
        height: usize,
        rows: Rows,
    ) -> Result<(), WorkspaceError> {
        let text = rows.text;
        self.set_rows(rows);
        let scrolloff = self.scrolloff;
        let (state, editable) = self.shown(pane)?;
        state.state.viewport.resize(height, text);
        state.state.reveal(
            editable.buffer,
            &state.selections,
            editable.state,
            scrolloff,
        );
        Ok(())
    }

    /// Lo que mide una línea de texto y cada heading. Si cambia, cada
    /// panel revisa su ventana.
    fn set_rows(&mut self, rows: Rows) {
        if self.documents.set_rows(rows) {
            for pane in self.panes.values_mut() {
                pane.state.viewport.heights_changed();
            }
        }
    }

    /// La rueda llevó `pane` hasta `top`; su cursor la sigue, como en vim,
    /// sin darle el foco. Un panel sin foco está en Normal.
    ///
    /// # Errors
    /// Si `pane` no existe o su documento tampoco, lo que sería un bug.
    pub fn scroll_to(&mut self, pane: PaneId, top: usize) -> Result<(), WorkspaceError> {
        if pane == self.focused() {
            return self.edit_pane(pane, None, |vim, target| {
                vim.scroll_to(top, target);
                Ok(())
            });
        }
        let scrolloff = self.scrolloff;
        let (state, editable) = self.shown(pane)?;
        state.state.scroll_to(
            top,
            editable.buffer,
            &mut state.selections,
            editable.state,
            scrolloff,
            Mode::Normal,
        );
        Ok(())
    }

    /// `pane`, para cambiarlo, y el documento que muestra.
    pub(super) fn shown(
        &mut self,
        pane: PaneId,
    ) -> Result<(&mut Pane, Editable<'_>), WorkspaceError> {
        let state = self
            .panes
            .get_mut(&pane)
            .ok_or(WorkspaceError::NoPane(pane))?;
        let id = state.document;
        let document = self
            .documents
            .get_mut(id)
            .ok_or(WorkspaceError::NoDocument(id))?;
        Ok((state, document.editable()))
    }
}

#[cfg(test)]
#[path = "scrolling_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "heights_tests.rs"]
mod heights_tests;
