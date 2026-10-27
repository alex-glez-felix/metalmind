//! La columna de la lista en la ventana: lee las tarjetas en el fondo, corre
//! las búsquedas del filtro fuera del hilo de la UI, como el finder, y abre
//! la nota elegida en el panel de edición.

use std::path::PathBuf;
use std::sync::Arc;

use gpui_kit::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, KeyDownEvent, Pixels,
    ScrollStrategy, Subscription, Task, UniformListScrollHandle, Window,
};
use metalmind_keymap::{Action, ListAction, context};
use metalmind_search::FuzzyMatcher;
use metalmind_vault::{EntryKind, VaultPath};

use super::cards::read_all;
use super::model::{NoteList, Prompt};
use super::source::ListSource;
use crate::actions::Dispatch;
use crate::finder::{Note, VaultChanged, VaultSession};
use crate::picker::{LineEdit, Step};
use crate::session::ListState;
use crate::workspace::{Axis, Placement, Workspace, shown_path};

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
    /// Al llegar las primeras tarjetas se elige la nota activa.
    reveal_on_load: bool,
    /// Lo mostrado cambió porque el usuario lo pidió (escribió en el
    /// filtro, cambió la fuente): al llegar, la vista va a la fila elegida.
    /// Un lote del watcher no mueve el scroll.
    scroll_on_show: bool,
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
        let list = NoteList::new(
            state.source.clone(),
            &state.filter,
            Arc::clone(&index),
            session.read(cx).tags(),
        );
        let changes = cx.subscribe(session, |view, _, VaultChanged(update), cx| {
            let (step, unread) = view.list.apply(update);
            view.step(step, cx);
            view.read_cards(unread, cx);
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
            reveal_on_load: true,
            scroll_on_show: false,
            _changes: changes,
            _workspace: workspace_changes,
        };
        let notes: Vec<VaultPath> = index
            .files()
            .filter(|entry| entry.kind == EntryKind::Note)
            .map(|entry| entry.path.clone())
            .collect();
        if notes.is_empty() {
            // Un vault sin notas no tiene nada que leer: la lista ya está.
            view.reveal_on_load = false;
            let step = view.list.set_cards(Vec::new());
            view.step(step, cx);
        }
        view.read_cards(notes, cx);
        view
    }

    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.chosen.or(self.by_width).unwrap_or(false)
    }

    /// Sin estado guardado, la lista arranca abierta solo en una ventana al
    /// menos de `min_width`. Se decide una vez y no se guarda: el próximo
    /// arranque vuelve a mirar el ancho.
    pub fn settle_visibility(&mut self, window_width: Pixels, min_width: Pixels) {
        self.by_width.get_or_insert(window_width >= min_width);
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
            self.reveal_active(cx);
        } else {
            self.list.close_prompt();
        }
        cx.notify();
        visible
    }

    pub fn set_source(&mut self, source: ListSource, cx: &mut Context<Self>) {
        self.chosen = Some(true);
        self.scroll_on_show = true;
        let step = self.list.set_source(source);
        self.step(step, cx);
    }

    /// Un tag renombrado o fusionado desde la app: si era la fuente, la
    /// lista lo sigue con su nombre nuevo.
    pub fn rename_tag(&mut self, from: &str, to: &str, cx: &mut Context<Self>) {
        let step = self.list.rename_tag(from, to);
        self.step(step, cx);
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
    fn step(&mut self, step: Step<Note>, cx: &mut Context<Self>) {
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

    fn scroll_to_selection(&self) {
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

    /// Las acciones de la lista; las demás siguen hacia la raíz de la ventana.
    pub(super) fn on_dispatch(
        &mut self,
        Dispatch(invocation): &Dispatch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Action::List(action) = invocation.action else {
            cx.propagate();
            return;
        };
        window.focus(&self.focus_handle, cx);
        match action {
            ListAction::SelectNext => self.list.select_next(1),
            ListAction::SelectPrevious => self.list.select_previous(1),
            ListAction::SelectFirst => self.list.select_first(),
            ListAction::SelectLast => self.list.select_last(),
            ListAction::Open => self.open_selected(Placement::Current, true, cx),
            ListAction::OpenSplitRight => {
                self.open_selected(Placement::Split(Axis::Row), true, cx);
            }
            ListAction::OpenSplitDown => {
                self.open_selected(Placement::Split(Axis::Column), true, cx);
            }
            ListAction::Filter => self.list.start_filter(),
            ListAction::Tag => {
                self.list.start_tagging();
            }
            ListAction::Confirm => self.confirm(cx),
            ListAction::Cancel => self.cancel(cx),
            ListAction::FocusEditor => cx.emit(NoteListEvent::FocusEditor),
            ListAction::FocusSidebar => cx.emit(NoteListEvent::FocusSidebar),
        }
        self.scroll_to_selection();
        cx.notify();
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        match self.list.prompt() {
            Some(Prompt::Filter) | None => self.list.close_prompt(),
            Some(Prompt::Tag) => {
                if let Some((note, tags)) = self.list.take_tags() {
                    cx.emit(NoteListEvent::TagNote { note, tags });
                }
            }
        }
    }

    /// Cierra la pregunta; sin pregunta, quita el filtro.
    fn cancel(&mut self, cx: &mut Context<Self>) {
        if self.list.prompt().is_some() {
            self.list.close_prompt();
            return;
        }
        self.scroll_on_show = true;
        // Al final, borrar hasta el principio es borrarlo todo.
        let _ = self.list.edit_filter(LineEdit::End);
        let step = self.list.edit_filter(LineEdit::DeleteToStart);
        self.step(step, cx);
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

    fn open_selected(&mut self, placement: Placement, focus_editor: bool, cx: &mut Context<Self>) {
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

    /// Lo que se teclea en el filtro o en los tags; `enter` y `esc` son
    /// acciones del registro.
    pub(super) fn on_key_down(
        &mut self,
        event: &KeyDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(prompt) = self.list.prompt() else {
            return;
        };
        let Some(edit) = line_edit(&event.keystroke) else {
            return;
        };
        cx.stop_propagation();
        match prompt {
            Prompt::Filter => {
                self.scroll_on_show = true;
                let step = self.list.edit_filter(edit);
                self.step(step, cx);
            }
            Prompt::Tag => {
                self.list.edit_tag_line(edit);
                cx.notify();
            }
        }
    }
}

/// La edición de una línea que pide `keystroke`, como en readline.
fn line_edit(keystroke: &gpui_kit::Keystroke) -> Option<LineEdit<'_>> {
    let modifiers = keystroke.modifiers;
    if modifiers.alt || modifiers.platform || modifiers.function {
        return None;
    }
    if modifiers.control {
        return match keystroke.key.as_str() {
            "u" => Some(LineEdit::DeleteToStart),
            "w" | "backspace" => Some(LineEdit::DeleteWordBackward),
            "a" => Some(LineEdit::Home),
            "e" => Some(LineEdit::End),
            _ => None,
        };
    }
    Some(match keystroke.key.as_str() {
        "backspace" => LineEdit::DeleteBackward,
        "delete" => LineEdit::DeleteForward,
        "left" => LineEdit::Left,
        "right" => LineEdit::Right,
        "home" => LineEdit::Home,
        "end" => LineEdit::End,
        _ => LineEdit::Insert(keystroke.key_char.as_deref()?),
    })
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
