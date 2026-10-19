//! Los comandos de `:` y `ctrl-w`: guardar, abrir, cerrar y los paneles.
//! Como en vim con `hidden`, cerrar o cambiar un panel deja su documento
//! oculto con sus cambios; solo cerrar la ventana se niega sin un `!`
//! ([`super::buffers`]).

use std::path::Path;
use std::rc::Rc;

use metalmind_buffer::Selection;
use metalmind_vault::SaveKind;
use metalmind_vim::{Clipboard, Command, Invocation, Mode};

use super::options;
use super::pane_tree::{Axis, Direction};
use super::{DocumentId, Message, Pane, PaneId, Workspace, WorkspaceError, clamp_cursor};
use crate::document::{Document, SaveError};
use crate::panel::PanelView;

/// Lo que la ventana tiene que hacer tras un comando.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
pub enum Effect {
    Stay,
    CloseWindow,
    OpenFinder,
    OpenTextSearch,
    OpenCommandPalette,
    /// Mostrar el panel lateral en esa vista, o esconderlo si ya la muestra.
    TogglePanel(PanelView),
    /// `ctrl-w` hacia un lado sin panel vecino: lo que haya fuera de los
    /// paneles de ese lado, como el panel lateral, puede tomar el foco.
    FocusPastEdge(Direction),
    /// `gd` o `enter` sobre un link cuyo nombre empata entre varias notas:
    /// la ventana muestra [`Workspace::link_choice`] para elegir una.
    ChooseLink,
}

impl Workspace {
    /// Ejecuta `invocation` en el panel enfocado y deja el aviso, si lo hay,
    /// en [`Workspace::message`]. Si el comando salió de una macro, la
    /// macro sigue en el panel que quedó con el foco, hasta su próximo
    /// comando o hasta terminar; como en vim, un error la corta.
    pub fn run(&mut self, invocation: &Invocation, clipboard: &mut dyn Clipboard) -> Effect {
        let mut invocation = invocation.clone();
        loop {
            let effect = self.run_one(&invocation);
            if effect != Effect::Stay {
                // Lo que quede de la macro no tiene a dónde ir: la ventana
                // se cierra o el picker se lleva las teclas.
                self.vim.abort_macro();
                return effect;
            }
            match self.resume_macro(clipboard) {
                Some(next) => invocation = next,
                None => return effect,
            }
        }
    }

    fn run_one(&mut self, invocation: &Invocation) -> Effect {
        let before = self.message.take();
        let effect = self.try_run(invocation).unwrap_or_else(|error| {
            self.report(&error);
            Effect::Stay
        });
        match &self.message {
            Some(Message::Error(_)) => self.vim.abort_macro(),
            Some(Message::Info(_)) => {}
            None => self.message = before,
        }
        effect
    }

    fn try_run(&mut self, invocation: &Invocation) -> Result<Effect, WorkspaceError> {
        let path = invocation.argument.as_deref().map(Path::new);
        match invocation.command {
            Command::Save => self.save(path, false).map(|_| Effect::Stay),
            Command::ForceSave => self.save(path, true).map(|_| Effect::Stay),
            Command::Quit => Ok(self.quit(false)),
            Command::ForceQuit => Ok(self.quit(true)),
            Command::SaveAndQuit => self.save_and_quit(path),
            Command::SaveAll => {
                self.save_all();
                Ok(Effect::Stay)
            }
            Command::QuitAll => Ok(self.quit_all(false)),
            Command::ForceQuitAll => Ok(self.quit_all(true)),
            Command::SaveAllAndQuit => Ok(if self.save_all() {
                self.quit_all(false)
            } else {
                Effect::Stay
            }),
            Command::Edit => self.edit(path, false).map(|()| Effect::Stay),
            Command::ForceEdit => self.edit(path, true).map(|()| Effect::Stay),
            Command::SplitRight => self.split(Axis::Row).map(|()| Effect::Stay),
            Command::SplitDown => self.split(Axis::Column).map(|()| Effect::Stay),
            Command::FocusLeft => self.move_focus(Direction::Left),
            Command::FocusDown => self.move_focus(Direction::Down),
            Command::FocusUp => self.move_focus(Direction::Up),
            Command::FocusRight => self.move_focus(Direction::Right),
            Command::FocusNext => {
                let next = self.tree.next();
                self.focus(next).map(|()| Effect::Stay)
            }
            Command::Only => {
                self.only();
                Ok(Effect::Stay)
            }
            Command::Set => self.set_options(invocation.argument.as_deref()),
            Command::OpenFinder => Ok(Effect::OpenFinder),
            Command::OpenTextSearch => Ok(Effect::OpenTextSearch),
            Command::OpenCommandPalette => Ok(Effect::OpenCommandPalette),
            Command::ToggleFolders => Ok(Effect::TogglePanel(PanelView::Folders)),
            Command::ToggleTags => Ok(Effect::TogglePanel(PanelView::Tags)),
            Command::FollowLink => self.follow_link(),
        }
    }

    /// Guarda el documento enfocado, o lo escribe en `path` como `:w ruta`;
    /// devuelve si se pudo. Con `force`, como `:w!`, pisa lo que haya en
    /// disco.
    fn save(&mut self, path: Option<&Path>, force: bool) -> Result<bool, WorkspaceError> {
        let id = self.pane(self.focused())?.document;
        if let Some(path) = path
            && self.loaded_elsewhere(id, path, force)
        {
            self.message = Some(Message::loaded_elsewhere());
            return Ok(false);
        }
        let disk = Rc::clone(&self.disk);
        let document = self.document_mut(id)?;
        let unnamed = document.path().is_none();
        let saved = match path {
            Some(path) => document.save_to(path, disk.as_ref(), force),
            None if force => document.overwrite(disk.as_ref(), SaveKind::Explicit),
            None => document.save(disk.as_ref(), SaveKind::Explicit),
        };

        let message = match saved {
            Ok(stats) => {
                let name =
                    path.map_or_else(|| document.display_name(), |p| p.display().to_string());
                Message::written(&name, stats)
            }
            Err(error) => {
                log::error!("{error}");
                // Una copia en otra ruta la nombra; el archivo propio ya se ve en la
                // barra, y un error de vim sobre lo recién escrito va tal cual.
                let other = path
                    .filter(|_| !error.is_vim())
                    .map(|p| p.display().to_string());
                Message::save_failed(other.as_deref(), &error)
            }
        };
        let saved = matches!(message, Message::Info(_));
        self.message = Some(message);
        // `:w otra-ruta` escribe una copia y la nota sigue con sus cambios: su
        // autoguardado sigue como estaba.
        if saved && !self.document(id)?.is_modified() {
            self.saved(id);
        }
        if saved && unnamed {
            self.adopt_file(id)?;
        }
        Ok(saved)
    }

    /// Un `[No Name]` al que `:w ruta` le dio archivo sigue con su lugar en
    /// la jumplist; lo que guardaba el archivo que antes tenía esa ruta ya no
    /// lleva a ningún lado.
    fn adopt_file(&mut self, id: DocumentId) -> Result<(), WorkspaceError> {
        let resolved = self.document(id)?.resolved_path();
        if let (Some(resolved), Some(file)) = (resolved, self.documents.file(id))
            && let Some(displaced) = self.files.adopt(file, &resolved)
        {
            self.vim.file_removed(displaced);
        }
        Ok(())
    }

    /// `:wq [ruta]`. Si `ruta` es otro archivo, el buffer sigue con sus
    /// cambios tras escribir la copia: en un split, el panel se cierra y
    /// el buffer queda oculto; en el último, no se cierra la ventana.
    fn save_and_quit(&mut self, path: Option<&Path>) -> Result<Effect, WorkspaceError> {
        if !self.save(path, false)? {
            return Ok(Effect::Stay);
        }
        Ok(self.quit(false))
    }

    /// Otro documento abierto es de `path`: escribirlo dejaría dos buffers
    /// sobre el mismo archivo o pisaría el de otro. Sin `!`, si el archivo ya
    /// existe gana el E13 de `save_to`; con `!` (`:w! ruta`) es E139 igual,
    /// como en nvim 0.12.6.
    fn loaded_elsewhere(&self, id: DocumentId, path: &Path, force: bool) -> bool {
        self.documents
            .find(path)
            .is_some_and(|other| other != id && (force || path.symlink_metadata().is_err()))
    }

    /// `:q` cierra el panel; el último cierra la ventana. Con `!`, como en
    /// nvim, el documento que ya no se ve en ningún panel se suelta con sus
    /// cambios en vez de quedar oculto.
    fn quit(&mut self, force: bool) -> Effect {
        if self.tree.len() == 1 {
            return self.quit_all(force);
        }
        let pane = self.focused();
        let shown = self.panes.get(&pane).map(|state| state.document);
        self.close(pane);
        if let Some(id) = shown.filter(|&id| force && self.panes_showing(id).next().is_none()) {
            self.forget(id);
        }
        Effect::Stay
    }

    /// `:qa`, o `:q` en el último panel.
    fn quit_all(&mut self, force: bool) -> Effect {
        if force || self.may_close_window() {
            Effect::CloseWindow
        } else {
            Effect::Stay
        }
    }

    pub(super) fn close(&mut self, pane: PaneId) {
        if self.tree.close(pane)
            && let Some(old) = self.panes.remove(&pane)
        {
            self.hide(old);
        }
    }

    /// `:e`: sin ruta, o con la del mismo archivo, lo relee; si no, el
    /// panel pasa a mostrar ese archivo y el de antes queda oculto, también
    /// con `:e!`, como en nvim. Una ruta relativa parte del directorio de
    /// trabajo, como en vim.
    pub(super) fn edit(&mut self, path: Option<&Path>, force: bool) -> Result<(), WorkspaceError> {
        let pane = self.focused();
        // Desde `:` ya se está en Normal; desde el finder puede quedar un
        // Insert, que termina antes, también si se niega.
        self.interrupt(pane)?;
        let current = self.pane(pane)?.document;
        let target = match path {
            Some(path) if !self.document(current)?.is_file(path) => path,
            _ => return self.reload(force),
        };
        self.push_jump(pane)?;
        self.show_file(pane, target)?;
        Ok(())
    }

    /// `:e` y `:e!` sin ruta: vuelve a leer el archivo. Los cursores de
    /// todos sus paneles se quedan en la misma línea y columna, o lo más
    /// cerca que se pueda. El historial empieza de nuevo.
    fn reload(&mut self, force: bool) -> Result<(), WorkspaceError> {
        let id = self.pane(self.focused())?.document;
        let document = self.document(id)?;
        if !force && document.is_modified() {
            self.message = Some(Message::unsaved());
            return Ok(());
        }
        let Some(path) = document.path() else {
            self.message = Some(Message::Error(SaveError::NoFileName.to_string()));
            return Ok(());
        };
        let reloaded = match Document::open(path) {
            Ok(reloaded) => reloaded,
            Err(error) => {
                self.message = Some(Message::Error(error.to_string()));
                return Ok(());
            }
        };
        self.replace_document(id, reloaded)?;
        self.message = Some(Message::opened(self.document(id)?));
        self.vim.cursor_moved();
        Ok(())
    }

    /// `id` pasa a ser `reloaded`, leído de nuevo del disco, con sus marks.
    /// Los cursores de sus paneles, también el del último que lo mostró si
    /// está oculto, se quedan en la misma línea y columna, o lo más cerca
    /// que se pueda. El historial empieza de nuevo.
    pub(super) fn replace_document(
        &mut self,
        id: DocumentId,
        mut reloaded: Document,
    ) -> Result<(), WorkspaceError> {
        reloaded.keep_marks_of(self.document(id)?)?;
        reloaded.set_rows(self.documents.rows());
        let old = std::mem::replace(self.document_mut(id)?, reloaded);
        self.saved(id);
        let buffer = self
            .documents
            .get(id)
            .ok_or(WorkspaceError::NoDocument(id))?
            .buffer();
        let shown = self
            .panes
            .values_mut()
            .filter(|pane| pane.document == id)
            .map(|pane| &mut pane.selections);
        let hidden = self.hidden.get_mut(&id).map(|last| &mut last.selections);
        for selections in shown.chain(hidden) {
            *selections = selections.map(|selection| {
                let line = old.buffer().line_of(selection.head);
                let column = old.buffer().column_of(selection.head);
                let head = buffer.offset_at_column(line, column);
                Selection::cursor(clamp_cursor(buffer, head, Mode::Normal))
            });
        }
        Ok(())
    }

    /// Como `:vsplit ruta` de vim: `path` en un panel nuevo, que se queda
    /// con el foco. El documento se abre antes de dividir, así que si no se
    /// puede leer no queda un panel de más; si ya está abierto (también si
    /// es el del panel enfocado) se comparte, con el cursor de un panel que
    /// lo muestra. Como al cambiar de panel, lo que estuviera a medias en el
    /// de antes termina como con `ctrl-c`.
    ///
    /// # Errors
    /// Solo si el estado está inconsistente, lo que sería un bug.
    pub fn open_in_split(&mut self, axis: Axis, path: &Path) -> Result<(), WorkspaceError> {
        let focused = self.focused();
        self.interrupt(focused)?;
        let numbers = self.pane(focused)?.numbers;
        let Some(id) = self.open_document(path) else {
            return Ok(());
        };
        let shown = self.pane_on(id, numbers)?;
        self.push_jump(focused)?;
        let jumps = self.pane(focused)?.jumps.clone();
        self.message = Some(Message::opened(self.document(shown.document)?));
        self.vim.cursor_moved();
        let new = self.tree.split(axis);
        self.panes.insert(new, Pane { jumps, ..shown });
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

    fn move_focus(&mut self, direction: Direction) -> Result<Effect, WorkspaceError> {
        match self.tree.neighbor(direction) {
            Some(pane) => self.focus(pane).map(|()| Effect::Stay),
            None => Ok(Effect::FocusPastEdge(direction)),
        }
    }

    /// `:set`, sobre el panel enfocado.
    fn set_options(&mut self, argument: Option<&str>) -> Result<Effect, WorkspaceError> {
        let pane = self.focused();
        let state = self
            .panes
            .get_mut(&pane)
            .ok_or(WorkspaceError::NoPane(pane))?;
        if let Err(error) = options::set(&mut state.numbers, argument) {
            self.message = Some(Message::Error(error.to_string()));
        }
        Ok(Effect::Stay)
    }

    /// `ctrl-w o`: cierra los demás paneles; sus documentos quedan ocultos.
    fn only(&mut self) {
        let keep = self.focused();
        for pane in self.tree.panes() {
            if pane != keep {
                self.close(pane);
            }
        }
    }
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
