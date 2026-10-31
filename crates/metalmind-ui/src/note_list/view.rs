//! La columna de la lista en la ventana: lee las tarjetas en el fondo, corre
//! las búsquedas del filtro fuera del hilo de la UI, como el finder, y abre
//! la nota elegida en el panel de edición.

use std::path::PathBuf;
use std::sync::Arc;

use gpui_kit::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Pixels, ScrollStrategy,
    Subscription, Task, UniformListScrollHandle, Window,
};
use metalmind_config::SavedView;
use metalmind_keymap::context;
use metalmind_search::FuzzyMatcher;
use metalmind_vault::{EntryKind, VaultPath};

use super::cards::read_all;
use super::model::{NoteList, Prompt};
use super::source::{FolderError, ListSource, folder_to_show};
use super::views::load as load_views;
use crate::finder::{Note, VaultChanged, VaultSession};
use crate::picker::{LineEdit, Step};
use crate::session::ListState;
use crate::workspace::{Placement, Workspace, shown_path};

/// Lo que la ventana tiene que hacer por la lista.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoteListEvent {
    /// Se abrió una nota en el panel de edición enfocado; con
    /// `focus_editor`, el foco pasa ahí.
    Opened {
        focus_editor: bool,
    },
    FocusEditor,
    /// `ctrl-w h`: el foco pasa al panel lateral, si se ve.
    FocusSidebar,
    /// Agregar `tags` al frontmatter de `note`.
    TagNote {
        note: VaultPath,
        tags: Vec<String>,
    },
}

pub struct NoteListView {
    pub(super) list: NoteList,
    /// Si el usuario la abrió o la plegó en este vault: es lo que se guarda.
    chosen: Option<bool>,
    /// Sin elección, lo que dijo el ancho de la ventana en el primer frame.
    by_width: Option<bool>,
    root: PathBuf,
    session: Entity<VaultSession>,
    workspace: Entity<Workspace>,
    pub(super) focus_handle: FocusHandle,
    pub(super) scroll: UniformListScrollHandle,
    /// Se presta a la búsqueda en curso y vuelve con sus resultados.
    matcher: Option<FuzzyMatcher>,
    search: Option<Task<()>>,
    /// Una lectura de tarjetas a la vez, para que una vieja no pise a una
    /// nueva de la misma nota; lo que llega mientras tanto espera aquí.
    reading: Option<Task<()>>,
    unread: Vec<VaultPath>,
    /// El preedit del IME o de una tecla muerta en el campo que se escribe.
    pub(super) marked: Option<String>,
    /// Al llegar las primeras tarjetas se elige la nota activa.
    reveal_on_load: bool,
    /// Cuántas veces eligió el usuario una fuente: `:vaultundo` no deshace una
    /// fuente que se eligió después del cambio.
    source_choices: u64,
    /// Las tarjetas se leen la primera vez que la lista se ve: plegada no
    /// le cuesta nada a la ventana.
    started: bool,
    /// Lo mostrado cambió porque el usuario lo pidió (escribió en el
    /// filtro, cambió la fuente): al llegar, la vista va a la fila elegida.
    /// Un lote del watcher no mueve el scroll.
    pub(super) scroll_on_show: bool,
    _changes: Subscription,
    _workspace: Subscription,
}

impl EventEmitter<NoteListEvent> for NoteListView {}

impl NoteListView {
    pub fn new(
        session: &Entity<VaultSession>,
        state: &ListState,
        workspace: Entity<Workspace>,
        cx: &mut Context<Self>,
    ) -> Self {
        let vault = session.read(cx).vault();
        let index = vault.snapshot();
        // Una vista guardada se muestra con la consulta que tiene hoy.
        let source = state.source.clone().resolve(&load_views(vault.root()).0);
        let list = NoteList::new(
            source,
            &state.filter,
            Arc::clone(&index),
            session.read(cx).tags(),
        );
        let changes = cx.subscribe(session, |view, _, VaultChanged(update), cx| {
            let (step, unread) = view.list.apply(update);
            view.step(step, cx);
            // Antes de la primera lectura no hay nada que releer: esa lee todo.
            if view.started {
                view.read_cards(unread, cx);
            }
        });
        // La nota activa se marca en la lista: cambia con el panel de edición.
        let workspace_changes = cx.observe(&workspace, |_, _, cx| cx.notify());
        let mut view = Self {
            list,
            chosen: state.visible,
            by_width: None,
            root: vault.root().to_owned(),
            session: session.clone(),
            workspace,
            focus_handle: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(),
            matcher: Some(FuzzyMatcher::paths()),
            search: None,
            reading: None,
            unread: Vec::new(),
            marked: None,
            reveal_on_load: true,
            source_choices: 0,
            started: false,
            scroll_on_show: false,
            _changes: changes,
            _workspace: workspace_changes,
        };
        if view.is_visible() {
            view.start(cx);
        }
        view
    }

    /// La primera lectura de las tarjetas, de todas las notas del vault.
    fn start(&mut self, cx: &mut Context<Self>) {
        if std::mem::replace(&mut self.started, true) {
            return;
        }
        let notes: Vec<VaultPath> = self
            .session
            .read(cx)
            .vault()
            .snapshot()
            .files()
            .filter(|entry| entry.kind == EntryKind::Note)
            .map(|entry| entry.path.clone())
            .collect();
        if notes.is_empty() {
            // Un vault sin notas no tiene nada que leer: la lista ya está.
            self.reveal_on_load = false;
            let step = self.list.set_cards(Vec::new());
            self.step(step, cx);
        }
        self.read_cards(notes, cx);
    }

    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.chosen.or(self.by_width).unwrap_or(false)
    }

    /// Sin estado guardado, la lista arranca abierta solo en una ventana al
    /// menos de `min_width`. Se decide una vez y no se guarda: el próximo
    /// arranque vuelve a mirar el ancho.
    pub fn settle_visibility(
        &mut self,
        window_width: Pixels,
        min_width: Pixels,
        cx: &mut Context<Self>,
    ) {
        self.by_width.get_or_insert(window_width >= min_width);
        if self.is_visible() {
            self.start(cx);
        }
    }

    #[must_use]
    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus_handle
    }

    #[must_use]
    pub fn state(&self) -> ListState {
        ListState {
            visible: self.chosen,
            source: self.list.source().clone(),
            filter: self.list.filter().text().to_owned(),
        }
    }

    /// Muestra la lista con la nota activa elegida, o la pliega. Devuelve si
    /// quedó visible.
    pub fn toggle(&mut self, cx: &mut Context<Self>) -> bool {
        let visible = !self.is_visible();
        self.chosen = Some(visible);
        if visible {
            self.start(cx);
            self.reveal_active(cx);
        } else {
            self.list.close_prompt();
            self.marked = None;
        }
        cx.notify();
        visible
    }

    pub fn set_source(&mut self, source: ListSource, cx: &mut Context<Self>) {
        self.show(source, None, cx);
    }

    /// Las notas de la carpeta `named` o, sin nombre, de la de la nota
    /// activa, con la nota activa elegida si es de ahí.
    ///
    /// # Errors
    /// Si no hay tal carpeta en el vault (ver [`folder_to_show`]).
    pub fn show_folder(
        &mut self,
        named: Option<&str>,
        cx: &mut Context<Self>,
    ) -> Result<(), FolderError> {
        let active = self.active(cx);
        let index = self.session.read(cx).vault().snapshot();
        let folder = folder_to_show(named, active.as_ref(), &index)?;
        self.show(ListSource::folder(&folder), active, cx);
        Ok(())
    }

    fn show(&mut self, source: ListSource, selected: Option<VaultPath>, cx: &mut Context<Self>) {
        self.chosen = Some(true);
        self.start(cx);
        // Lo elegido gana a la nota activa que se iba a elegir al cargar.
        self.reveal_on_load = false;
        self.source_choices += 1;
        self.scroll_on_show = true;
        let step = self.list.set_source(source, selected);
        self.step(step, cx);
    }

    /// Muestra la vista que se acaba de guardar desde la lista, con el
    /// filtro vacío.
    pub fn show_saved_view(&mut self, view: &SavedView, cx: &mut Context<Self>) {
        self.source_choices += 1;
        self.scroll_on_show = true;
        let step = self.list.show_saved_view(view);
        self.step(step, cx);
    }

    /// Las vistas del vault cambiaron: la fuente sigue a la suya.
    pub fn follow_views(
        &mut self,
        views: &[SavedView],
        renamed: Option<(&str, &str)>,
        cx: &mut Context<Self>,
    ) {
        if let Some(step) = self.list.follow_views(views, renamed) {
            self.step(step, cx);
        }
    }

    /// El preedit del IME en el campo que se escribe.
    pub(super) fn marked(&self) -> &str {
        self.marked.as_deref().unwrap_or("")
    }

    #[must_use]
    pub fn source_choices(&self) -> u64 {
        self.source_choices
    }

    /// Un tag renombrado o fusionado desde la app: si era la fuente, la
    /// lista lo sigue con su nombre nuevo. Devuelve si la siguió.
    pub fn rename_tag(&mut self, from: &str, to: &str, cx: &mut Context<Self>) -> bool {
        let Some(step) = self.list.rename_tag(from, to) else {
            return false;
        };
        self.step(step, cx);
        true
    }

    fn reveal_active(&mut self, cx: &Context<Self>) {
        if let Some(active) = self.active(cx)
            && let Some(row) = self.list.reveal(&active)
        {
            self.scroll.scroll_to_item(row, ScrollStrategy::Nearest);
        }
    }

    /// La nota del panel de edición enfocado, si es del vault.
    pub(super) fn active(&self, cx: &App) -> Option<VaultPath> {
        let document = self.workspace.read(cx).focused_document().ok()?;
        self.session.read(cx).vault_path(document.path()?)
    }

    /// Lee en el fondo las tarjetas de `paths`, después de las que ya se
    /// están leyendo.
    fn read_cards(&mut self, paths: Vec<VaultPath>, cx: &mut Context<Self>) {
        self.unread.extend(paths);
        if self.reading.is_some() || self.unread.is_empty() {
            return;
        }
        let paths = std::mem::take(&mut self.unread);
        let root = self.root.clone();
        let read = cx.background_spawn(async move { read_all(&root, &paths) });
        self.reading = Some(cx.spawn(async move |this, cx| {
            let cards = read.await;
            // Si la lista ya no existe, no hay a quién dárselas.
            let _ = this.update(cx, |view, cx| {
                view.reading = None;
                let step = view.list.set_cards(cards);
                view.step(step, cx);
                if std::mem::take(&mut view.reveal_on_load) {
                    view.reveal_active(cx);
                }
                view.read_cards(Vec::new(), cx);
            });
        }));
    }

    /// Corre la búsqueda que pidió el modelo, como el finder.
    pub(super) fn step(&mut self, step: Step<Note>, cx: &mut Context<Self>) {
        if self.scroll_on_show && self.list.is_current() {
            self.scroll_on_show = false;
            self.scroll_to_selection();
        }
        cx.notify();
        let Some(job) = step.job else {
            return;
        };
        let mut matcher = self.matcher.take().unwrap_or_else(FuzzyMatcher::paths);
        let running = cx.background_spawn(async move {
            let found = job.run(&mut matcher);
            (found, matcher)
        });
        self.search = Some(cx.spawn(async move |this, cx| {
            let (found, matcher) = running.await;
            let _ = this.update(cx, |view, cx| {
                view.matcher = Some(matcher);
                let step = view.list.finish(found);
                view.step(step, cx);
            });
        }));
    }

    pub(super) fn scroll_to_selection(&self) {
        if let Some(row) = self.list.selected_row() {
            self.scroll.scroll_to_item(row, ScrollStrategy::Nearest);
        }
    }

    pub(super) fn key_context(&self) -> &'static str {
        match self.list.prompt() {
            Some(_) => context::NOTE_LIST_INPUT,
            None => context::NOTE_LIST,
        }
    }

    /// Edita el campo de la pregunta abierta.
    pub(super) fn edit_prompt(&mut self, edit: LineEdit<'_>, cx: &mut Context<Self>) {
        match self.list.prompt() {
            Some(Prompt::Filter) => {
                self.scroll_on_show = true;
                let step = self.list.edit_filter(edit);
                self.step(step, cx);
            }
            Some(Prompt::Tag) => {
                self.list.edit_tag_line(edit);
                cx.notify();
            }
            None => {}
        }
    }

    /// La fila `row`, con el ratón: se elige y se abre; con doble clic, el
    /// foco pasa a la nota.
    pub(super) fn click(
        &mut self,
        row: usize,
        clicks: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle, cx);
        self.list.select(row);
        self.open_selected(Placement::Current, clicks > 1, cx);
        cx.notify();
    }

    pub(super) fn open_selected(
        &mut self,
        placement: Placement,
        focus_editor: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(note) = self.list.selected_note() else {
            return;
        };
        let file = shown_path(&self.root.join(note.as_str()));
        let opened = self.workspace.update(cx, |workspace, cx| {
            cx.notify();
            workspace
                .open_file(&file, placement)
                .unwrap_or_else(|error| {
                    workspace.report(&error);
                    false
                })
        });
        if opened {
            cx.emit(NoteListEvent::Opened { focus_editor });
        }
    }
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
