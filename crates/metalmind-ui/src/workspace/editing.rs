//! La entrada de un panel: teclas, texto del IME, ratón y portapapeles.

use metalmind_buffer::{ByteOffset, Goal, Selection, Selections};
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
        let outcome = self.edit_pane(pane, Some(&mut *clipboard), |vim, target| {
            Ok(vim.feed(key, target)?)
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
        let outcome = self.edit_pane(pane, Some(&mut *clipboard), |vim, target| {
            Ok(vim.execute(action, target)?)
        })?;
        self.note(&outcome);
        self.settle(outcome, clipboard)
    }

    /// Sigue la macro pausada en el panel enfocado, saltando de archivo
    /// cuando lo pida; devuelve su próximo comando, si llega a uno.
    pub(super) fn resume_macro(&mut self, clipboard: &mut dyn Clipboard) -> Option<Invocation> {
        loop {
            let resumed = self
                .edit_pane(self.focused(), Some(&mut *clipboard), |vim, target| {
                    Ok(vim.resume_macro(target)?)
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
                Ok(Some(KeyOutcome::Dispatch(invocation))) => return Some(invocation),
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

    /// Cambia `typed`, lo que acaba de escribirse antes del cursor, por
    /// `text`, como si se tecleara: entra en el mismo paso de undo que el
    /// resto de la pasada por Insert y `.` lo repite. Como el completado de
    /// nvim, si `text` sigue a lo escrito solo se teclea el resto.
    pub(super) fn type_over(
        &mut self,
        pane: PaneId,
        typed: &str,
        text: &str,
        clipboard: &mut dyn Clipboard,
    ) -> Result<(), WorkspaceError> {
        let rest = text.strip_prefix(typed);
        if rest.is_none() {
            for _ in typed.chars() {
                self.feed(pane, Key::Backspace, clipboard)?;
            }
        }
        self.type_text(pane, rest.unwrap_or(text), clipboard)?;
        Ok(())
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
        self.vim.cursor_moved();
        let mode = self.vim.mode();
        let (state, editable) = self.shown(pane)?;
        let selection = if anchor == head {
            Selection::cursor(clamp_cursor(editable.buffer, head, mode))
        } else {
            Selection {
                anchor,
                head,
                goal: Goal::None,
            }
        };
        state.selections = Selections::single(selection);
        state
            .state
            .reveal(editable.buffer, &state.selections, editable.state, 0);
        Ok(())
    }

    /// El botón del ratón baja en `offset`: lleva el cursor ahí. Dice si
    /// cayó en la casilla de una tarea (`- [ ]`) estando en Normal: soltarlo
    /// ahí sin arrastrar la marca o desmarca ([`Workspace::toggle_checkbox`]),
    /// como en Obsidian.
    ///
    /// # Errors
    /// Si `pane` no existe.
    pub fn press(&mut self, pane: PaneId, offset: ByteOffset) -> Result<bool, WorkspaceError> {
        let normal = self.vim.mode() == Mode::Normal;
        self.select(pane, offset, offset)?;
        Ok(normal && self.on_checkbox(pane, offset)?)
    }

    /// `editor::toggle_checkbox` en la línea del cursor de `pane`.
    ///
    /// # Errors
    /// Si `pane` no existe o una edición resulta inválida, lo que sería un bug.
    pub fn toggle_checkbox(
        &mut self,
        pane: PaneId,
        clipboard: &mut dyn Clipboard,
    ) -> Result<(), WorkspaceError> {
        self.execute(pane, Action::ToggleCheckbox, clipboard)?;
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
            .state()
            .styles()
            .and_then(|styles| styles.line(line));
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
        let outcome = self.edit_pane(pane, Some(clipboard), |vim, target| {
            vim.use_register(SystemRegister::Clipboard);
            Ok(vim.execute(action, target)?)
        })?;
        self.note(&outcome);
        Ok(())
    }

    /// Pega en el cursor de `pane`, o en la línea de comandos si está
    /// abierta, como [`metalmind_vim::Vim::paste_text`].
    ///
    /// # Errors
    /// Si `pane` no existe o la edición resulta inválida, lo que sería un bug.
    pub fn paste(&mut self, pane: PaneId, text: &str) -> Result<(), WorkspaceError> {
        if text.is_empty() {
            return Ok(());
        }
        self.focus(pane)?;
        self.edit_pane(pane, None, |vim, target| {
            vim.paste_text(text, target)?;
            Ok(())
        })
    }
}

#[cfg(test)]
#[path = "editing_tests.rs"]
mod tests;
