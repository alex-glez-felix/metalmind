//! Cambiar el documento de un panel: la única función que lo hace, para
//! `:e`, el árbol, el finder, seguir un link y los saltos de la jumplist.

use std::path::Path;

use super::{DocumentId, Pane, PaneId, PaneOptions, Workspace, WorkspaceError};
use crate::document::Document;
use crate::workspace::Message;

impl Workspace {
    /// `pane` pasa a mostrar `path`, abriéndolo si no está abierto. Como
    /// vim con `hidden`, no se niega por cambios sin guardar: el documento
    /// de antes queda oculto, con sus cambios y su cursor. Lo que estuviera
    /// a medias termina como con `ctrl-c`. Devuelve el documento que
    /// muestra; `None` si no se pudo leer, con el error en
    /// [`Workspace::message`].
    ///
    /// # Errors
    /// Solo si el estado está inconsistente, lo que sería un bug.
    pub fn show_file(
        &mut self,
        pane: PaneId,
        path: &Path,
    ) -> Result<Option<DocumentId>, WorkspaceError> {
        self.interrupt(self.focused())?;
        let Some(id) = self.open_document(path) else {
            return Ok(None);
        };
        self.show_document(pane, id)?;
        Ok(Some(id))
    }

    /// El documento abierto de `path`, o uno nuevo leído del disco. Si no se
    /// puede leer, `None` con el error en [`Workspace::message`].
    pub(super) fn open_document(&mut self, path: &Path) -> Option<DocumentId> {
        if let Some(id) = self.documents.find(path) {
            return Some(id);
        }
        match Document::open(path) {
            Ok(document) => {
                let on_disk = document.exists_on_disk();
                let id = self.insert_document(document);
                if let Some(file) = self.documents.file(id).filter(|_| on_disk) {
                    self.vim.file_restored(file);
                }
                Some(id)
            }
            Err(error) => {
                self.messages.push(Message::Error(error.to_string()));
                None
            }
        }
    }

    /// `pane` pasa a mostrar `id`, con el cursor de un panel que ya lo
    /// muestra o, si estaba oculto, del último que lo mostró; sus números,
    /// su jumplist y el tamaño de su ventana se quedan. El documento de antes queda oculto y, con
    /// autoguardado, se guarda.
    pub(super) fn show_document(
        &mut self,
        pane: PaneId,
        id: DocumentId,
    ) -> Result<(), WorkspaceError> {
        // Lo pendiente (modo, operador, `:`) es del motor, que edita el panel
        // enfocado aunque se cambie otro.
        self.interrupt(self.focused())?;
        let current = self.pane(pane)?;
        if current.document == id {
            return Ok(());
        }
        let (options, jumps) = (current.options, current.state.jumps.clone());
        let window = &current.state.viewport;
        let (height, row) = (window.height(), window.row());
        let mut shown = self.pane_on(id, options)?;
        shown.state.jumps = jumps;
        // El tamaño es del panel, como en vim: quien mueva el cursor antes
        // del próximo layout ya ve la ventana con su alto.
        shown.state.viewport.resize(height, row);
        self.messages.record(Message::opened(self.document(id)?));
        self.vim.cursor_moved();
        if let Some(old) = self.panes.insert(pane, shown) {
            let left = old.document;
            self.hide(old);
            self.autosave_document(left);
        }
        Ok(())
    }

    /// Un panel con `options` sobre `id`, con el cursor de uno que ya lo
    /// muestra o, si está oculto, del último que lo mostró, tras releerlo.
    /// La jumplist es la de un panel nuevo: quien llama pone la suya.
    pub(super) fn pane_on(
        &mut self,
        id: DocumentId,
        options: PaneOptions,
    ) -> Result<Pane, WorkspaceError> {
        if let Some(shown) = self.panes.values().find(|pane| pane.document == id) {
            return Ok(Pane {
                options,
                ..shown.clone()
            });
        }
        let mut pane = Pane::at_start(id, options, self.tabstop);
        if self.hidden.contains_key(&id) {
            self.refresh(id)?;
        }
        if let Some(last) = self.hidden.remove(&id) {
            pane.selections = last.selections;
            pane.state.viewport = last.viewport;
        }
        Ok(pane)
    }

    /// Un documento sin cambios pasa a tener lo que dice el disco, como con
    /// `autoread` de nvim: al volver a uno oculto o cuando avisa el watcher,
    /// se vea o no. Un cambio de fuera (git, Syncthing) no deja texto viejo,
    /// y un archivo que apareció se carga. Si no cambió, conserva su
    /// historial; las marks y el cursor guardado siguen a su línea. Si ya
    /// no se puede leer o lo borraron, se queda como estaba: el buffer es
    /// la única copia.
    pub(super) fn refresh(&mut self, id: DocumentId) -> Result<(), WorkspaceError> {
        let document = self.document(id)?;
        let Some(path) = document.path().filter(|_| !document.is_modified()) else {
            return Ok(());
        };
        let fresh = match Document::open(path) {
            Ok(fresh) => fresh,
            Err(error) => {
                log::warn!("no se releyó {}: {error}", path.display());
                return Ok(());
            }
        };
        let deleted = document.exists_on_disk() && !fresh.exists_on_disk();
        if deleted || fresh.same_on_disk(document) {
            return Ok(());
        }
        self.replace_document(id, fresh)
    }
}
