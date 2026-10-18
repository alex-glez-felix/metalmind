//! La entrada de un panel: teclas, texto del IME, ratón y portapapeles.

use nota_buffer::{Bias, ByteOffset, Edit, Goal, Selection, Selections};
use nota_vim::{Action, Clipboard, Key, KeyOutcome};

use super::{Message, PaneId, Workspace, WorkspaceError, clamp_cursor};

impl Workspace {
    /// Una tecla en `pane`, que pasa a tener el foco si no lo tenía. Los
    /// comandos que devuelve los ejecuta quien aloja el editor, con
    /// [`Workspace::run`] o por su acción con nombre. `clipboard` es lo que
    /// leen y escriben los registros `"+` y `"*`.
    ///
    /// # Errors
    /// Si `pane` no existe o una edición resulta inválida, lo que sería un bug.
    pub fn feed(
        &mut self,
        pane: PaneId,
        key: Key,
        clipboard: &mut dyn Clipboard,
    ) -> Result<KeyOutcome, WorkspaceError> {
        self.focus(pane)?;
        let outcome = self.edit_pane(pane, |vim, buffer, selections| {
            Ok(vim.feed_with_clipboard(key, buffer, selections, clipboard)?)
        })?;
        match &outcome {
            KeyOutcome::Executed(Action::CommandLine) => self.message = None,
            KeyOutcome::InvalidCommand(error) => {
                self.message = Some(Message::Error(error.to_string()));
            }
            _ => {}
        }
        Ok(outcome)
    }

    /// Texto ya compuesto por el teclado o el IME: cada carácter entra al
    /// motor como una tecla. Devuelve lo que pasó con el último.
    ///
    /// # Errors
    /// Si `pane` no existe o una edición resulta inválida, lo que sería un bug.
    pub fn type_text(
        &mut self,
        pane: PaneId,
        text: &str,
        clipboard: &mut dyn Clipboard,
    ) -> Result<Option<KeyOutcome>, WorkspaceError> {
        let mut last = None;
        for c in text.chars() {
            last = Some(self.feed(pane, Key::Char(c), clipboard)?);
        }
        Ok(last)
    }

    /// Selección hecha con el ratón, con `head` exclusivo. Un clic en Normal
    /// no deja el cursor sobre el salto de línea, y en Insert descarta la
    /// repetición del count, como en vim.
    ///
    /// # Errors
    /// Si `pane` no existe.
    pub fn select(
        &mut self,
        pane: PaneId,
        anchor: ByteOffset,
        head: ByteOffset,
    ) -> Result<(), WorkspaceError> {
        self.focus(pane)?;
        self.edit_pane(pane, |vim, buffer, selections| {
            vim.cursor_moved();
            let selection = if anchor == head {
                Selection::cursor(clamp_cursor(buffer, head, vim.mode()))
            } else {
                Selection {
                    anchor,
                    head,
                    goal: Goal::None,
                }
            };
            *selections = Selections::single(selection);
            Ok(())
        })
    }

    /// # Errors
    /// Si `pane` no existe o la selección no cae en el buffer, lo que sería
    /// un bug.
    pub fn selected_text(&self, pane: PaneId) -> Result<Option<String>, WorkspaceError> {
        let view = self.view(pane)?;
        if view.selection.is_cursor() {
            return Ok(None);
        }
        Ok(Some(view.buffer().text_in(view.selection.range())?))
    }

    /// La línea del cursor con su salto, como la copia `yy`.
    ///
    /// # Errors
    /// Si `pane` no existe o el buffer está inconsistente, lo que sería un bug.
    pub fn current_line(&self, pane: PaneId) -> Result<String, WorkspaceError> {
        let view = self.view(pane)?;
        let mut line = view
            .document
            .line_text(view.buffer().line_of(view.cursor()))?;
        line.push('\n');
        Ok(line)
    }

    /// Pega en el cursor, o en la línea de comandos si está abierta. En
    /// Insert entra como lo tecleado: `u` lo deshace junto con lo escrito y
    /// el count lo repite. En Normal es un paso propio y el cursor queda
    /// sobre el último carácter pegado.
    ///
    /// # Errors
    /// Si `pane` no existe o la edición resulta inválida, lo que sería un bug.
    pub fn paste(&mut self, pane: PaneId, text: &str) -> Result<(), WorkspaceError> {
        if text.is_empty() {
            return Ok(());
        }
        self.focus(pane)?;
        self.edit_pane(pane, |vim, buffer, selections| {
            if vim.mode().takes_text() {
                vim.insert_text(text, buffer, selections)?;
                return Ok(());
            }
            vim.cursor_moved();
            buffer.begin_transaction(selections.clone())?;
            let at = selections.primary().head;
            let map = buffer.apply(vec![Edit::insert(at, text)])?;
            let end = map.map(at, Bias::After);
            *selections = Selections::single(Selection::cursor(buffer.prev_boundary(end)));
            buffer.commit_transaction(selections.clone())?;
            Ok(())
        })
    }
}
