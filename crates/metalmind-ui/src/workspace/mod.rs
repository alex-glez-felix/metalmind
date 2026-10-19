//! Lo que hay abierto en una ventana, sin nada de GPUI: los documentos, el
//! árbol de paneles con el cursor de cada uno y el motor vim.
//!
//! Hay un solo [`Vim`] para todos los paneles, como en vim, donde el modo,
//! los registros, `.` y `;` son globales: lo copiado en un buffer se pega en
//! otro. Las marks, en cambio, son de cada documento, y la ventana
//! ([`Viewport`]) de cada panel: el motor las toma prestadas mientras edita.
//! Cambiar de panel lo deja en Normal antes, como un `ctrl-c`.
//!
//! Como vim con `hidden`, el default de nvim, un documento que ningún panel
//! muestra queda oculto, con sus cambios, su historial y su cursor.

mod autosave;
mod buffers;
mod commands;
mod documents;
mod editing;
mod file_ids;
mod files;
mod gutter_marks;
mod jumps;
mod line_numbers;
mod links;
mod message;
mod options;
mod pane_tree;
mod scrolling;
mod show;
mod source_lines;

use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::rc::Rc;

use metalmind_buffer::{Buffer, BufferError, ByteOffset, Goal, Selection, Selections, Splice};
use metalmind_vim::{
    Action, FileId, Indent, JumpList, Keymap, Layout, Markdown, Mode, Pattern, Prompt, Uniform,
    Viewport, Vim, VimError,
};

pub use self::autosave::{Autosave, SaveState, SystemClock};
pub use self::commands::Effect;
pub use self::documents::DocumentId;
use self::documents::Documents;
use self::file_ids::FileIds;
pub use self::files::{LinePosition, Placement, shown_path};
pub use self::gutter_marks::{GutterMarks, MarkLabel};
pub use self::line_numbers::LineNumbers;
pub use self::links::LinkChoice;
use self::links::VaultLinks;
pub use self::message::Message;
pub use self::pane_tree::{Axis, Direction, Node, PaneId, PaneTree};
pub use self::source_lines::SourceLines;
use crate::disk::{Disk, Files};
use crate::document::{Document, Editable};

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

/// `viewport` muestra `head` con `scrolloff` líneas alrededor, con lo que
/// mide cada línea: antes se sigue lo editado y se mide lo que va a mirar.
fn reveal(
    viewport: &mut Viewport,
    buffer: &mut Buffer,
    layout: Option<&mut Layout>,
    head: ByteOffset,
    scrolloff: usize,
) {
    if let Some(layout) = layout {
        layout.sync(buffer);
        layout.ensure_window(buffer, viewport, head, scrolloff, 0);
        viewport.reveal(buffer, head, scrolloff, layout);
    } else {
        let uniform = Uniform(viewport.row());
        viewport.reveal(buffer, head, scrolloff, &uniform);
    }
}

/// Lo que muestra un panel.
#[derive(Clone, Debug)]
struct Pane {
    document: DocumentId,
    selections: Selections,
    numbers: LineNumbers,
    viewport: Viewport,
    /// Como en vim, la jumplist es de cada panel; un split copia la suya.
    jumps: JumpList,
}

/// Dónde estaba el cursor de un documento oculto, para volver ahí. La
/// jumplist y los números no van: son del panel, no del documento.
#[derive(Clone, Debug)]
struct LastView {
    selections: Selections,
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
            jumps: JumpList::default(),
        }
    }
}

#[derive(Debug)]
pub struct Workspace {
    documents: Documents,
    files: FileIds,
    panes: BTreeMap<PaneId, Pane>,
    tree: PaneTree,
    /// Los documentos ocultos, con el cursor del último panel que los
    /// mostró.
    hidden: BTreeMap<DocumentId, LastView>,
    vim: Vim,
    /// Las líneas que se dejan visibles alrededor del cursor en cada panel.
    scrolloff: usize,
    /// Las columnas hasta la siguiente parada de un tab, en cada panel.
    tabstop: usize,
    /// El margen muestra las marks de cada línea.
    gutter_marks: bool,
    message: Option<Message>,
    links: Option<VaultLinks>,
    link_choice: Option<LinkChoice>,
    /// Donde se escriben los documentos.
    disk: Rc<dyn Disk>,
    /// `None` si `editor.autosave` está apagado: todo es como en vim.
    autosave: Option<Autosave>,
}

/// Lo que vale igual en todos los paneles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    /// Las líneas que se dejan visibles alrededor del cursor.
    pub scrolloff: usize,
    /// Las columnas hasta la siguiente parada de un tab, y las que corren
    /// `>` y `<`: Metalmind no tiene un `shiftwidth` aparte.
    pub tabstop: usize,
    /// `>` y `<` escriben solo espacios.
    pub expandtab: bool,
    /// Qué partes de la edición de markdown están encendidas.
    pub markdown: Markdown,
    /// El margen muestra las marks de cada línea.
    pub gutter_marks: bool,
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
    /// El indicador del autoguardado; `None` si está apagado.
    pub save: Option<SaveState>,
}

/// Lo que se está escribiendo tras `:`, `/` o `?`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandLine<'a> {
    pub prompt: Prompt,
    pub typed: &'a str,
}

impl PaneView<'_> {
    /// Si hay que avisar de cambios sin guardar: el `[+]` de vim o, con
    /// autoguardado, lo que diga el indicador.
    #[must_use]
    pub fn unsaved(&self) -> bool {
        match self.save {
            Some(state) => state != SaveState::Saved,
            None => self.document.is_modified(),
        }
    }

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
    pub fn new(document: Document, numbers: LineNumbers, options: Options) -> Self {
        let Options {
            scrolloff,
            tabstop,
            expandtab,
            gutter_marks,
            markdown,
        } = options;
        let message = Message::opened(&document);
        let mut documents = Documents::default();
        let mut files = FileIds::default();
        let file = files.of_document(&document);
        let id = documents.insert(document, file);
        let tree = PaneTree::new();
        let mut vim = Vim::new();
        vim.set_scrolloff(scrolloff);
        vim.set_indent(Indent {
            shiftwidth: NonZeroUsize::MIN.saturating_add(tabstop.saturating_sub(1)),
            expandtab,
        });
        vim.set_markdown(markdown);
        Self {
            documents,
            files,
            panes: BTreeMap::from([(tree.focus(), Pane::at_start(id, numbers, tabstop))]),
            tree,
            hidden: BTreeMap::new(),
            vim,
            scrolloff,
            tabstop,
            gutter_marks,
            message: Some(message),
            links: None,
            link_choice: None,
            disk: Rc::new(Files),
            autosave: None,
        }
    }

    /// Donde se escriben los documentos: el vault de la ventana, para que
    /// cada guardado deje su versión en el historial.
    pub fn set_disk(&mut self, disk: Rc<dyn Disk>) {
        self.disk = disk;
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
        let document = self.document(state.document)?;
        Ok(PaneView {
            document,
            save: self.save_state(state.document, document),
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

    fn insert_document(&mut self, document: Document) -> DocumentId {
        let file = self.files.of_document(&document);
        self.documents.insert(document, file)
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
        let file = self
            .documents
            .file(id)
            .ok_or(WorkspaceError::NoDocument(id))?;
        let document = self
            .documents
            .get_mut(id)
            .ok_or(WorkspaceError::NoDocument(id))?;
        let Editable {
            buffer,
            marks,
            layout,
        } = document.editable();
        self.vim.swap_marks(marks);
        self.vim.swap_viewport(&mut state.viewport);
        self.vim.swap_jumps(&mut state.jumps);
        self.vim.set_file(file);
        self.vim.swap_layout(layout);
        let result = edit(&mut self.vim, buffer, &mut state.selections);
        self.vim.swap_layout(layout);
        self.vim.catch_up(buffer);
        self.vim.swap_jumps(&mut state.jumps);
        self.vim.swap_viewport(&mut state.viewport);
        self.vim.swap_marks(marks);
        let head = state.selections.primary().head;
        reveal(
            &mut state.viewport,
            buffer,
            layout.as_mut(),
            head,
            scrolloff,
        );
        let splices = buffer.take_splices();
        document.settle();
        self.follow(pane, id, file, &splices)?;
        if !splices.is_empty() {
            self.edited(id);
        }
        result
    }

    /// Los cursores de los otros paneles de `id` y lo que guardan sus
    /// jumplists de `file` siguen a lo editado.
    fn follow(
        &mut self,
        edited: PaneId,
        id: DocumentId,
        file: FileId,
        splices: &[Splice],
    ) -> Result<(), WorkspaceError> {
        if splices.is_empty() {
            return Ok(());
        }
        let Editable { buffer, layout, .. } = self
            .documents
            .get_mut(id)
            .ok_or(WorkspaceError::NoDocument(id))?
            .editable();
        let scrolloff = self.scrolloff;
        for (_, other) in self.panes.iter_mut().filter(|(pane, _)| **pane != edited) {
            other.jumps.adjust(file, splices);
        }
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
            reveal(
                &mut other.viewport,
                buffer,
                layout.as_mut(),
                head,
                scrolloff,
            );
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

    /// `pane`, que ya no está en el árbol o muestra otra cosa, mostraba
    /// `old`: si ningún otro panel muestra ese documento, queda oculto con
    /// su cursor, como los buffers de vim con `hidden`, con sus cambios, sus
    /// marks y su historial. Un `[No Name]` sin cambios se suelta, como vim
    /// lo reusa.
    fn hide(&mut self, old: Pane) {
        let id = old.document;
        if self.panes_showing(id).next().is_some() {
            return;
        }
        let disposable = self
            .documents
            .get(id)
            .is_some_and(|document| document.path().is_none() && !document.is_modified());
        if disposable {
            self.forget(id);
        } else {
            self.hidden.insert(
                id,
                LastView {
                    selections: old.selections,
                    viewport: old.viewport,
                },
            );
        }
    }

    /// Suelta `id`, aunque tenga cambios.
    fn forget(&mut self, id: DocumentId) {
        self.documents.remove(id);
        self.hidden.remove(&id);
        self.saved(id);
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
#[path = "source_lines_tests.rs"]
mod source_lines_tests;
#[cfg(test)]
mod test_session;
#[cfg(test)]
#[path = "workspace_tests.rs"]
mod tests;
