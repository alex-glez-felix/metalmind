//! Lo que hay abierto en una ventana, sin nada de GPUI: los documentos, el
//! árbol de paneles con el cursor de cada uno y el motor vim.
//!
//! Hay un solo [`Vim`] para todos los paneles, como en vim, donde el modo,
//! los registros, `.` y `;` son globales: lo copiado en un buffer se pega en
//! otro. Las marks, en cambio, son de cada documento, y la ventana
//! ([`Viewport`]) de cada panel: el motor las toma prestadas mientras edita. Cambiar de panel lo deja en Normal antes,
//! como un `ctrl-c`.

mod commands;
mod documents;
mod editing;
mod files;
mod line_numbers;
mod message;
mod options;
mod pane_tree;
mod scrolling;

use std::collections::BTreeMap;

use metalmind_buffer::{Buffer, BufferError, ByteOffset, Goal, Selection, Selections, Splice};
use metalmind_vim::{Action, Keymap, Mode, Pattern, Prompt, Viewport, Vim, VimError};

pub use self::commands::Effect;
pub use self::documents::DocumentId;
use self::documents::Documents;
pub use self::files::{Placement, shown_path};
pub use self::line_numbers::LineNumbers;
pub use self::message::Message;
pub use self::pane_tree::{Axis, Direction, Node, PaneId, PaneTree};
use crate::document::Document;

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error(transparent)]
    Vim(#[from] VimError),
    #[error(transparent)]
    Buffer(#[from] BufferError),
    #[error("no existe el panel {0}")]
    NoPane(PaneId),
    #[error("no existe el documento {0}")]
    NoDocument(DocumentId),
}

/// Lo que muestra un panel.
#[derive(Clone, Debug)]
struct Pane {
    document: DocumentId,
    selections: Selections,
    numbers: LineNumbers,
    viewport: Viewport,
}

impl Pane {
    fn at_start(document: DocumentId, numbers: LineNumbers, tabstop: usize) -> Self {
        let mut viewport = Viewport::default();
        viewport.set_tabstop(tabstop);
        Self {
            document,
            selections: Selections::single(Selection::cursor(ByteOffset::ZERO)),
            numbers,
            viewport,
        }
    }
}

#[derive(Debug)]
pub struct Workspace {
    documents: Documents,
    panes: BTreeMap<PaneId, Pane>,
    tree: PaneTree,
    vim: Vim,
    /// Las líneas que se dejan visibles alrededor del cursor en cada panel.
    scrolloff: usize,
    /// Las columnas hasta la siguiente parada de un tab, en cada panel.
    tabstop: usize,
    message: Option<Message>,
}

/// Un panel visto desde la UI.
#[derive(Clone, Copy, Debug)]
pub struct PaneView<'a> {
    pub document: &'a Document,
    pub selection: Selection,
    /// Qué líneas se ven; la vista pinta desde [`Viewport::top`].
    pub viewport: Viewport,
    pub focused: bool,
    /// El modo de vim si tiene el foco; los demás se ven en Normal.
    pub mode: Mode,
    /// El registro de la macro que se graba, solo en el panel con el foco.
    pub recording: Option<char>,
    pub command_line: Option<CommandLine<'a>>,
    pub numbers: LineNumbers,
    /// El patrón a resaltar, en todos los paneles como `hlsearch`, hasta
    /// un `:noh`.
    pub search: Option<&'a Pattern>,
}

/// Lo que se está escribiendo tras `:`, `/` o `?`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandLine<'a> {
    pub prompt: Prompt,
    pub typed: &'a str,
}

impl PaneView<'_> {
    #[must_use]
    pub fn buffer(&self) -> &Buffer {
        self.document.buffer()
    }

    #[must_use]
    pub fn cursor(&self) -> ByteOffset {
        self.selection.head
    }
}

impl Workspace {
    /// Un panel con `document` y los números `numbers`, avisando como vim
    /// al abrirlo.
    #[must_use]
    pub fn new(document: Document, numbers: LineNumbers, scrolloff: usize, tabstop: usize) -> Self {
        let message = Message::opened(&document);
        let mut documents = Documents::default();
        let id = documents.insert(document);
        let tree = PaneTree::new();
        let mut vim = Vim::new();
        vim.set_scrolloff(scrolloff);
        Self {
            documents,
            panes: BTreeMap::from([(tree.focus(), Pane::at_start(id, numbers, tabstop))]),
            tree,
            vim,
            scrolloff,
            tabstop,
            message: Some(message),
        }
    }

    #[must_use]
    pub fn tree(&self) -> &PaneTree {
        &self.tree
    }

    #[must_use]
    pub fn focused(&self) -> PaneId {
        self.tree.focus()
    }

    #[must_use]
    pub fn message(&self) -> Option<&Message> {
        self.message.as_ref()
    }

    pub fn set_message(&mut self, message: Message) {
        self.message = Some(message);
    }

    /// Las teclas de vim, para mostrar el atajo de cada acción.
    #[must_use]
    pub fn keymap(&self) -> &Keymap {
        self.vim.keymap()
    }

    pub fn report(&mut self, error: &WorkspaceError) {
        log::error!("{error}");
        self.message = Some(Message::Error(error.to_string()));
    }

    /// # Errors
    /// Si `pane` no existe o su documento tampoco, lo que sería un bug.
    pub fn view(&self, pane: PaneId) -> Result<PaneView<'_>, WorkspaceError> {
        let state = self.pane(pane)?;
        let focused = pane == self.focused();
        Ok(PaneView {
            document: self.document(state.document)?,
            selection: *state.selections.primary(),
            viewport: state.viewport,
            focused,
            mode: if focused {
                self.vim.mode()
            } else {
                Mode::Normal
            },
            recording: self.vim.recording().filter(|_| focused),
            command_line: self.command_line().filter(|_| focused),
            numbers: state.numbers,
            search: self.vim.search_highlight(),
        })
    }

    /// El documento del panel enfocado.
    ///
    /// # Errors
    /// Solo si el estado está inconsistente, lo que sería un bug.
    pub fn focused_document(&self) -> Result<&Document, WorkspaceError> {
        self.view(self.focused()).map(|view| view.document)
    }

    /// Le da el foco a `pane`. Lo que estuviera a medias en el anterior (un
    /// Insert, un operador, la línea de comandos) termina como con `ctrl-c`.
    ///
    /// # Errors
    /// Si `pane` no existe.
    pub fn focus(&mut self, pane: PaneId) -> Result<(), WorkspaceError> {
        let previous = self.focused();
        if pane == previous {
            return Ok(());
        }
        if !self.tree.contains(pane) {
            return Err(WorkspaceError::NoPane(pane));
        }
        self.interrupt(previous)?;
        self.tree.set_focus(pane);
        Ok(())
    }

    /// Lo que esté a medias en `pane` (un Insert, un operador, la línea de
    /// comandos) termina como con `ctrl-c`: es lo que pasa antes de que el
    /// panel deje el foco o cambie de documento.
    fn interrupt(&mut self, pane: PaneId) -> Result<(), WorkspaceError> {
        self.edit_pane(pane, |vim, buffer, selections| {
            vim.execute(Action::Interrupt, buffer, selections)?;
            Ok(())
        })
    }

    fn command_line(&self) -> Option<CommandLine<'_>> {
        Some(CommandLine {
            prompt: self.vim.command_line_prompt()?,
            typed: self.vim.command_line()?,
        })
    }

    fn pane(&self, pane: PaneId) -> Result<&Pane, WorkspaceError> {
        self.panes.get(&pane).ok_or(WorkspaceError::NoPane(pane))
    }

    fn document(&self, id: DocumentId) -> Result<&Document, WorkspaceError> {
        self.documents.get(id).ok_or(WorkspaceError::NoDocument(id))
    }

    fn document_mut(&mut self, id: DocumentId) -> Result<&mut Document, WorkspaceError> {
        self.documents
            .get_mut(id)
            .ok_or(WorkspaceError::NoDocument(id))
    }

    /// Corre `edit` sobre el buffer, las marks, la ventana y los cursores de
    /// `pane`, que queda mostrando su cursor; después, los cursores de los
    /// otros paneles del mismo documento siguen al texto.
    fn edit_pane<T>(
        &mut self,
        pane: PaneId,
        edit: impl FnOnce(&mut Vim, &mut Buffer, &mut Selections) -> Result<T, WorkspaceError>,
    ) -> Result<T, WorkspaceError> {
        self.edit_pane_with(pane, self.scrolloff, edit)
    }

    /// Como [`Workspace::edit_pane`], mostrando el cursor con `scrolloff`
    /// líneas alrededor.
    fn edit_pane_with<T>(
        &mut self,
        pane: PaneId,
        scrolloff: usize,
        edit: impl FnOnce(&mut Vim, &mut Buffer, &mut Selections) -> Result<T, WorkspaceError>,
    ) -> Result<T, WorkspaceError> {
        let state = self
            .panes
            .get_mut(&pane)
            .ok_or(WorkspaceError::NoPane(pane))?;
        let id = state.document;
        let document = self
            .documents
            .get_mut(id)
            .ok_or(WorkspaceError::NoDocument(id))?;
        let (buffer, marks) = document.editable();
        self.vim.swap_marks(marks);
        self.vim.swap_viewport(&mut state.viewport);
        let result = edit(&mut self.vim, buffer, &mut state.selections);
        self.vim.swap_viewport(&mut state.viewport);
        self.vim.swap_marks(marks);
        let head = state.selections.primary().head;
        state.viewport.reveal(buffer, head, scrolloff);
        let splices = buffer.take_splices();
        self.follow(pane, id, &splices)?;
        result
    }

    fn follow(
        &mut self,
        edited: PaneId,
        id: DocumentId,
        splices: &[Splice],
    ) -> Result<(), WorkspaceError> {
        if splices.is_empty() {
            return Ok(());
        }
        let buffer = self
            .documents
            .get(id)
            .ok_or(WorkspaceError::NoDocument(id))?
            .buffer();
        let scrolloff = self.scrolloff;
        for (_, other) in self
            .panes
            .iter_mut()
            .filter(|(pane, other)| **pane != edited && other.document == id)
        {
            other.selections = other.selections.map(|selection| {
                let anchor = Splice::map_all(splices, selection.anchor);
                let head = Splice::map_all(splices, selection.head);
                normal_selection(buffer, anchor, head)
            });
            let head = other.selections.primary().head;
            other.viewport.reveal(buffer, head, scrolloff);
        }
        Ok(())
    }

    /// Paneles que muestran `id`.
    fn panes_showing(&self, id: DocumentId) -> impl Iterator<Item = PaneId> + '_ {
        self.panes
            .iter()
            .filter(move |(_, pane)| pane.document == id)
            .map(|(&pane, _)| pane)
    }

    /// Cerrar o cambiar el documento de `pane` perdería cambios: está
    /// modificado y ningún otro panel lo muestra.
    fn would_lose_changes(&self, pane: PaneId) -> Result<bool, WorkspaceError> {
        let id = self.pane(pane)?.document;
        let shown_elsewhere = self.panes_showing(id).any(|other| other != pane);
        Ok(!shown_elsewhere && self.document(id)?.is_modified())
    }

    /// Suelta los documentos que ya no muestra ningún panel.
    fn drop_unshown(&mut self) {
        let unshown: Vec<DocumentId> = self
            .documents
            .iter()
            .map(|(id, _)| id)
            .filter(|&id| self.panes_showing(id).next().is_none())
            .collect();
        for id in unshown {
            self.documents.remove(id);
        }
    }
}

/// Una selección en Normal: sin cursor sobre el salto de línea.
fn normal_selection(buffer: &Buffer, anchor: ByteOffset, head: ByteOffset) -> Selection {
    if anchor == head {
        Selection::cursor(clamp_cursor(buffer, head, Mode::Normal))
    } else {
        Selection {
            anchor,
            head,
            goal: Goal::None,
        }
    }
}

/// Dentro del texto y, salvo en Insert, no sobre el salto de línea, como en
/// vim.
fn clamp_cursor(buffer: &Buffer, offset: ByteOffset, mode: Mode) -> ByteOffset {
    let offset = offset.min(buffer.len());
    let line = buffer.line_of(offset);
    let line_end = buffer.line_end(line);
    let on_newline = offset == line_end && line_end > buffer.line_start(line);
    if on_newline && mode != Mode::Insert {
        buffer.prev_boundary(line_end)
    } else {
        offset
    }
}

#[cfg(test)]
mod panes_oracle;
#[cfg(test)]
mod test_session;
#[cfg(test)]
#[path = "workspace_tests.rs"]
mod tests;
