//! La entrada de un panel: teclas, texto del IME, ratón y portapapeles.

use metalmind_buffer::{Bias, ByteOffset, Edit, Goal, Selection, Selections};
use metalmind_syntax::Conceal;
use metalmind_vim::{
    Action, Clipboard, Invocation, Key, KeyOutcome, Mode, Operator, PasteSide, SystemRegister,
};

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
        self.note(&outcome);
        self.settle(outcome, clipboard)
    }

    /// Ejecuta `action` en `pane` como si se hubiera tecleado su atajo; así
    /// la paleta invoca las acciones del motor.
    ///
    /// # Errors
    /// Si `pane` no existe o una edición resulta inválida, lo que sería un bug.
    pub fn execute(
        &mut self,
        pane: PaneId,
        action: Action,
        clipboard: &mut dyn Clipboard,
    ) -> Result<KeyOutcome, WorkspaceError> {
        self.focus(pane)?;
        let outcome = self.edit_pane(pane, |vim, buffer, selections| {
            Ok(vim.execute_with_clipboard(action, buffer, selections, clipboard)?)
        })?;
        self.note(&outcome);
        self.settle(outcome, clipboard)
    }

    /// Sigue la macro pausada en el panel enfocado, saltando de archivo
    /// cuando lo pida; devuelve su próximo comando, si llega a uno.
    pub(super) fn resume_macro(&mut self, clipboard: &mut dyn Clipboard) -> Option<Invocation> {
        loop {
            let resumed = self
                .edit_pane(self.focused(), |vim, buffer, selections| {
                    Ok(vim.resume_macro(buffer, selections, clipboard)?)
                })
                .and_then(|resumed| match resumed {
                    Some(KeyOutcome::SwitchFile(landing)) => {
                        self.land(self.focused(), landing).map(|landed| {
                            if !landed {
                                self.vim.abort_macro();
                            }
                            Some(KeyOutcome::SwitchFile(landing))
                        })
                    }
                    resumed => Ok(resumed),
                });
            match resumed {
                Ok(Some(KeyOutcome::Command(invocation))) => return Some(invocation),
                Ok(Some(KeyOutcome::SwitchFile(_))) => {}
                Ok(Some(outcome)) => {
                    self.note(&outcome);
                    return None;
                }
                Ok(None) => return None,
                Err(error) => {
                    self.vim.abort_macro();
                    self.report(&error);
                    return None;
                }
            }
        }
    }

    /// El aviso que deja una tecla. Abrir la línea de comandos o encontrar
    /// lo buscado borra el anterior; lo demás no lo toca.
    fn note(&mut self, outcome: &KeyOutcome) {
        self.message = match outcome {
            KeyOutcome::Executed(
                Action::CommandLine
                | Action::Search { .. }
                | Action::SubmitCommand
                | Action::SearchNext { .. }
                | Action::SearchWord { .. },
            ) => None,
            KeyOutcome::InvalidCommand(error) => Some(Message::Error(error.to_string())),
            KeyOutcome::SearchFailed(error) => Some(Message::Error(error.to_string())),
            KeyOutcome::SearchWrapped(wrap) => Some(Message::wrapped(*wrap)),
            KeyOutcome::Substituted(substitution) => Message::substituted(*substitution),
            KeyOutcome::GlobalMissed(miss) => Some(Message::Info(miss.to_string())),
            _ => return,
        };
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
    /// repetición del count, como en vim. Como vim al arrastrar, muestra el
    /// cursor sin `scrolloff`: si no, la ventana se movería a cada paso y la
    /// selección se escaparía del ratón.
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
        self.edit_pane_with(pane, 0, |vim, buffer, selections| {
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

    /// Un clic en `offset`: lleva el cursor ahí y, si cae en la casilla de
    /// una tarea (`- [ ]`) en Normal, la marca o desmarca, como en Obsidian.
    ///
    /// # Errors
    /// Si `pane` no existe o una edición resulta inválida, lo que sería un bug.
    pub fn click(
        &mut self,
        pane: PaneId,
        offset: ByteOffset,
        clipboard: &mut dyn Clipboard,
    ) -> Result<(), WorkspaceError> {
        self.select(pane, offset, offset)?;
        if self.vim.mode() == Mode::Normal && self.on_checkbox(pane, offset)? {
            self.execute(pane, Action::ToggleCheckbox, clipboard)?;
        }
        Ok(())
    }

    /// Si `offset` cae en una casilla, según el estilo con que se pinta.
    fn on_checkbox(&self, pane: PaneId, offset: ByteOffset) -> Result<bool, WorkspaceError> {
        let view = self.view(pane)?;
        let buffer = view.document.buffer();
        let line = buffer.line_of(offset);
        let index = offset.get() - buffer.line_start(line).get();
        let style = view
            .document
            .layout()
            .and_then(|layout| layout.styles().line(line));
        Ok(style.is_some_and(|style| {
            style.spans.iter().any(|span| {
                matches!(span.conceal, Some(Conceal::Checkbox { .. }))
                    && span.range.contains(&index)
            })
        }))
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

    /// `ctrl-shift-c`. En Visual es `"+y`, así que copia lo seleccionado
    /// con sus dos extremos, por líneas con `V`, y sale a Normal como vim.
    /// Si no, copia la selección del ratón o la línea del cursor.
    ///
    /// # Errors
    /// Si `pane` no existe o una edición resulta inválida, lo que sería un bug.
    pub fn copy(
        &mut self,
        pane: PaneId,
        clipboard: &mut dyn Clipboard,
    ) -> Result<(), WorkspaceError> {
        if self.in_visual(pane)? {
            return self.with_clipboard_register(pane, Action::Operator(Operator::Yank), clipboard);
        }
        let text = match self.selected_text(pane)? {
            Some(text) => text,
            None => self.current_line(pane)?,
        };
        clipboard.write(SystemRegister::Clipboard, text);
        Ok(())
    }

    /// `ctrl-shift-v`. En Visual es `"+P`: lo pegado reemplaza la selección
    /// sin tocar los registros. Si no, pega en el cursor con
    /// [`Workspace::paste`].
    ///
    /// # Errors
    /// Si `pane` no existe o una edición resulta inválida, lo que sería un bug.
    pub fn paste_clipboard(
        &mut self,
        pane: PaneId,
        clipboard: &mut dyn Clipboard,
    ) -> Result<(), WorkspaceError> {
        if self.in_visual(pane)? {
            return self.with_clipboard_register(pane, Action::Paste(PasteSide::Before), clipboard);
        }
        match clipboard.read(SystemRegister::Clipboard) {
            Some(text) => self.paste(pane, &text),
            None => Ok(()),
        }
    }

    fn in_visual(&self, pane: PaneId) -> Result<bool, WorkspaceError> {
        Ok(matches!(self.view(pane)?.mode, Mode::Visual(_)))
    }

    /// `action` con `"+`, descartando lo que el motor tuviera pendiente.
    fn with_clipboard_register(
        &mut self,
        pane: PaneId,
        action: Action,
        clipboard: &mut dyn Clipboard,
    ) -> Result<(), WorkspaceError> {
        self.focus(pane)?;
        let outcome = self.edit_pane(pane, |vim, buffer, selections| {
            Ok(vim.execute_with_system_register(
                SystemRegister::Clipboard,
                action,
                buffer,
                selections,
                clipboard,
            )?)
        })?;
        self.note(&outcome);
        Ok(())
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

#[cfg(test)]
#[path = "editing_tests.rs"]
mod tests;
