//! Los comandos de `:` y `ctrl-w`: guardar, abrir, cerrar y los paneles.
//! Como en vim sin `hidden`, un documento vive mientras algún panel lo
//! muestre, así que nada que cierre o cambie un panel puede perder cambios
//! sin un `!`.

use std::path::Path;

use nota_buffer::Selection;
use nota_vim::{Command, Invocation, Mode};

use super::pane_tree::{Axis, Direction};
use super::{DocumentId, Message, Pane, PaneId, Workspace, WorkspaceError, clamp_cursor};
use crate::document::Document;

/// Lo que la ventana tiene que hacer tras un comando.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
pub enum Effect {
    Stay,
    CloseWindow,
}

impl Workspace {
    /// Ejecuta `invocation` en el panel enfocado y deja el aviso, si lo hay,
    /// en [`Workspace::message`].
    pub fn run(&mut self, invocation: &Invocation) -> Effect {
        match self.try_run(invocation) {
            Ok(effect) => effect,
            Err(error) => {
                self.report(&error);
                Effect::Stay
            }
        }
    }

    /// Si se puede cerrar la ventana sin perder cambios; si no, lo avisa.
    pub fn may_close_window(&mut self) -> bool {
        let focused = self.focused_document().ok();
        let Some((_, unsaved)) = self.documents.iter().find(|(_, d)| d.is_modified()) else {
            return true;
        };
        let focused_unsaved = focused.is_some_and(Document::is_modified);
        self.message = Some(if focused_unsaved {
            Message::unsaved("usa :q! para cerrar igual")
        } else {
            Message::unsaved_in(unsaved)
        });
        false
    }

    fn try_run(&mut self, invocation: &Invocation) -> Result<Effect, WorkspaceError> {
        let path = invocation.argument.as_deref().map(Path::new);
        match invocation.command {
            Command::Save => self.save().map(|_| Effect::Stay),
            Command::Quit => self.quit(false),
            Command::ForceQuit => self.quit(true),
            Command::SaveAndQuit => {
                if self.save()? {
                    self.quit(false)
                } else {
                    Ok(Effect::Stay)
                }
            }
            Command::Edit => self.edit(path, false).map(|()| Effect::Stay),
            Command::ForceEdit => self.edit(path, true).map(|()| Effect::Stay),
            Command::SplitRight => self.split(Axis::Row).map(|()| Effect::Stay),
            Command::SplitDown => self.split(Axis::Column).map(|()| Effect::Stay),
            Command::FocusLeft => self.move_focus(Direction::Left).map(|()| Effect::Stay),
            Command::FocusDown => self.move_focus(Direction::Down).map(|()| Effect::Stay),
            Command::FocusUp => self.move_focus(Direction::Up).map(|()| Effect::Stay),
            Command::FocusRight => self.move_focus(Direction::Right).map(|()| Effect::Stay),
            Command::FocusNext => {
                let next = self.tree.next();
                self.focus(next).map(|()| Effect::Stay)
            }
            Command::Only => self.only().map(|()| Effect::Stay),
        }
    }

    /// Guarda el documento enfocado; devuelve si se pudo.
    fn save(&mut self) -> Result<bool, WorkspaceError> {
        let id = self.pane(self.focused())?.document;
        let document = self.document_mut(id)?;
        let message = match document.save() {
            Ok(stats) => Message::written(document, stats),
            Err(error) => {
                log::error!("{error}");
                Message::Error(error.to_string())
            }
        };
        let saved = matches!(message, Message::Info(_));
        self.message = Some(message);
        Ok(saved)
    }

    /// `:q` cierra el panel; el último cierra la ventana.
    fn quit(&mut self, force: bool) -> Result<Effect, WorkspaceError> {
        let pane = self.focused();
        if self.tree.len() == 1 {
            let closes = force || self.may_close_window();
            return Ok(if closes {
                Effect::CloseWindow
            } else {
                Effect::Stay
            });
        }
        if !force && self.would_lose_changes(pane)? {
            self.message = Some(Message::unsaved("usa :q! para cerrar igual"));
            return Ok(Effect::Stay);
        }
        self.close(pane);
        Ok(Effect::Stay)
    }

    fn close(&mut self, pane: PaneId) {
        if self.tree.close(pane) {
            self.panes.remove(&pane);
            self.drop_unshown();
        }
    }

    /// `:e`: sin ruta, o con la del mismo archivo, lo relee; si no, el
    /// panel pasa a mostrar ese archivo. Una ruta relativa parte del
    /// directorio de trabajo, como en vim.
    fn edit(&mut self, path: Option<&Path>, force: bool) -> Result<(), WorkspaceError> {
        let pane = self.focused();
        let current = self.pane(pane)?.document;
        let target = match path {
            Some(path) if !self.document(current)?.is_file(path) => path,
            _ => return self.reload(force),
        };
        if !force && self.would_lose_changes(pane)? {
            self.message = Some(Message::unsaved("usa :e! para descartarlos"));
            return Ok(());
        }
        let shown = match self.documents.find(target) {
            Some(id) => self.pane_on(id),
            None => match Document::open(target) {
                Ok(document) => Pane::at_start(self.documents.insert(document)),
                Err(error) => {
                    self.message = Some(Message::Error(error.to_string()));
                    return Ok(());
                }
            },
        };
        self.message = Some(Message::opened(self.document(shown.document)?));
        self.vim.cursor_moved();
        self.panes.insert(pane, shown);
        self.drop_unshown();
        Ok(())
    }

    /// Un panel más sobre `id`, con el cursor de uno que ya lo muestra.
    fn pane_on(&self, id: DocumentId) -> Pane {
        self.panes
            .values()
            .find(|pane| pane.document == id)
            .cloned()
            .unwrap_or_else(|| Pane::at_start(id))
    }

    /// `:e` y `:e!` sin ruta: vuelve a leer el archivo. Los cursores de
    /// todos sus paneles se quedan en la misma línea y columna, o lo más
    /// cerca que se pueda. El historial empieza de nuevo.
    fn reload(&mut self, force: bool) -> Result<(), WorkspaceError> {
        let id = self.pane(self.focused())?.document;
        let document = self.document(id)?;
        if !force && document.is_modified() {
            self.message = Some(Message::unsaved("usa :e! para descartarlos"));
            return Ok(());
        }
        let reloaded = match Document::open(document.path()) {
            Ok(reloaded) => reloaded,
            Err(error) => {
                self.message = Some(Message::Error(error.to_string()));
                return Ok(());
            }
        };
        let old = std::mem::replace(self.document_mut(id)?, reloaded);
        let buffer = self
            .documents
            .get(id)
            .ok_or(WorkspaceError::NoDocument(id))?
            .buffer();
        for pane in self.panes.values_mut().filter(|pane| pane.document == id) {
            pane.selections = pane.selections.map(|selection| {
                let line = old.buffer().line_of(selection.head);
                let column = old.buffer().column_of(selection.head);
                let head = buffer.offset_at_column(line, column);
                Selection::cursor(clamp_cursor(buffer, head, Mode::Normal))
            });
        }
        self.message = Some(Message::opened(self.document(id)?));
        self.vim.cursor_moved();
        Ok(())
    }

    /// El panel nuevo muestra lo mismo, con el mismo cursor, y se queda
    /// con el foco.
    fn split(&mut self, axis: Axis) -> Result<(), WorkspaceError> {
        let copy = self.pane(self.focused())?.clone();
        let new = self.tree.split(axis);
        self.panes.insert(new, copy);
        Ok(())
    }

    fn move_focus(&mut self, direction: Direction) -> Result<(), WorkspaceError> {
        match self.tree.neighbor(direction) {
            Some(pane) => self.focus(pane),
            None => Ok(()),
        }
    }

    /// `ctrl-w o`: cierra los demás paneles, salvo los que se llevarían
    /// cambios sin guardar, como vim sin `hidden`.
    fn only(&mut self) -> Result<(), WorkspaceError> {
        let keep = self.focused();
        let mut kept_changes = false;
        for pane in self.tree.panes() {
            if pane == keep {
                continue;
            }
            if self.would_lose_changes(pane)? {
                kept_changes = true;
            } else {
                self.close(pane);
            }
        }
        if kept_changes {
            self.message = Some(Message::others_unsaved());
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
