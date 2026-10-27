//! Las listas del autocompletado en el editor: la de `#` (05 §3) se abre
//! al escribir un tag y cambia lo escrito por su forma establecida; la de
//! `[[`, al escribir el destino de un wikilink, y escribe con qué nombre
//! llega el link a la nota elegida. Las dos se pintan bajo lo escrito (o
//! encima, si no cabe) y se aceptan con `tab` o un clic.

use std::sync::Arc;

use gpui_kit::{
    AnyElement, App, Bounds, Context, InteractiveElement as _, IntoElement, KeyContext,
    MouseButton, ParentElement as _, Pixels, Point, StatefulInteractiveElement as _, Window,
    anchored, size,
};
use metalmind_keymap::{EditorAction, View, context};
use metalmind_search::FuzzyMatcher;
use metalmind_syntax::TagPlace;
use metalmind_vault::VaultPath;
use metalmind_vault::tags::{TagCompletion, TagKey, Tags};

use super::Editor;
use super::completion_menu::{CompletionMenu, MenuMetrics};
use super::cursor::cursor_position;
use super::layout::FrameLayout;
use crate::actions::{AppKeymap, EDITOR_CONTEXT};
use crate::clipboard::SystemClipboard;
use crate::completion::{LinkCompletions, NoteCandidates, Selection, TagCompletions};
use crate::finder::Note;
use crate::theme::Theme;
use crate::workspace::{Message, PaneId, TypedLink, TypedTag, Workspace, WorkspaceError};

mod rows;

/// La lista abierta.
enum List {
    Tags(TagCompletions),
    Links(LinkCompletions),
}

impl List {
    fn selection(&self) -> &Selection {
        match self {
            Self::Tags(list) => list.selection(),
            Self::Links(list) => list.selection(),
        }
    }

    fn selection_mut(&mut self) -> &mut Selection {
        match self {
            Self::Tags(list) => list.selection_mut(),
            Self::Links(list) => list.selection_mut(),
        }
    }

    fn typed(&self) -> &str {
        match self {
            Self::Tags(list) => list.typed(),
            Self::Links(list) => list.typed(),
        }
    }

    fn len(&self) -> usize {
        match self {
            Self::Tags(list) => list.rows().len(),
            Self::Links(list) => list.rows().len(),
        }
    }
}

/// Lo que el editor guarda para el autocompletado.
pub(super) struct Completion {
    list: Option<List>,
    /// Los candidatos de unos tags: se arman de nuevo solo si llegan otros.
    tags: Option<(Arc<Tags>, Arc<[TagCompletion]>)>,
    /// Las notas de `[[`: se arman de nuevo solo si cambian.
    notes: Option<Arc<NoteCandidates>>,
    /// Para nombres de tags.
    names: FuzzyMatcher,
    /// Para rutas de notas.
    paths: FuzzyMatcher,
}

impl Default for Completion {
    fn default() -> Self {
        Self {
            list: None,
            tags: None,
            notes: None,
            names: FuzzyMatcher::text(),
            paths: FuzzyMatcher::paths(),
        }
    }
}

impl Completion {
    pub(super) fn is_open(&self) -> bool {
        self.list.is_some()
    }

    pub(super) fn close(&mut self) {
        self.list = None;
    }

    fn tag_candidates(&mut self, tags: Arc<Tags>) -> Arc<[TagCompletion]> {
        match &self.tags {
            Some((cached, candidates)) if Arc::ptr_eq(cached, &tags) => Arc::clone(candidates),
            _ => {
                let candidates: Arc<[TagCompletion]> = tags.completions().into();
                self.tags = Some((tags, Arc::clone(&candidates)));
                candidates
            }
        }
    }
}

impl Completion {
    /// Un lote del vault trae otra lista de notas aunque no cambie (un
    /// autoguardado): convertirlas para filtrar se hace solo si cambió.
    fn note_candidates(&mut self, notes: Arc<[Note]>) -> Arc<NoteCandidates> {
        match &self.notes {
            Some(cached) if cached.has(&notes) => Arc::clone(cached),
            _ => {
                let candidates = Arc::new(NoteCandidates::new(notes));
                self.notes = Some(Arc::clone(&candidates));
                candidates
            }
        }
    }
}

impl Editor {
    /// El del editor y, con una lista abierta, el de sus atajos.
    pub(super) fn key_context(&self) -> KeyContext {
        let mut key_context = KeyContext::new_with_defaults();
        key_context.add(EDITOR_CONTEXT);
        if self.completion.is_open() {
            key_context.add(context::COMPLETION);
        }
        key_context
    }

    /// Tras cualquier cambio del workspace o del vault, la lista abierta
    /// sigue a lo que se está escribiendo con los tags y las notas de
    /// ahora, o se cierra.
    pub(super) fn refresh_completion(&mut self, cx: &App) {
        if self.completion.is_open() {
            self.update_completion(cx);
        }
    }

    /// Tras escribir texto: abre, filtra o cierra la lista según el link o
    /// el tag que se está escribiendo. Si lo escrito no cambió, la fila que
    /// eligió el usuario sigue elegida.
    pub(super) fn update_completion(&mut self, cx: &App) {
        let typed = Typed::in_pane(self.workspace.read(cx), self.pane)
            .inspect_err(|error| log::error!("{error}"))
            .ok()
            .flatten();
        self.completion.list = match typed {
            Some(Typed::Link(link)) => self.link_list(&link, cx),
            Some(Typed::Tag(tag)) => self.tag_list(&tag, cx),
            None => None,
        };
    }

    /// La lista de `[[`: solo en una nota del vault, que es desde donde se
    /// calcula cómo nombrar a las demás.
    fn link_list(&mut self, typed: &TypedLink, cx: &App) -> Option<List> {
        let source = self.note_path(cx)?;
        let notes = self.session.read(cx).notes();
        let candidates = self.completion.note_candidates(notes);
        let matcher = &mut self.completion.paths;
        match self.completion.list.take() {
            Some(List::Links(list)) if list.is_for(&candidates, &typed.typed, &source) => {
                Some(list)
            }
            Some(List::Links(list)) if list.is_among(&candidates, &source) => {
                list.retyped(&typed.typed, matcher)
            }
            Some(List::Links(list)) if list.typed() == typed.typed && *list.source() == source => {
                list.with_candidates(candidates, matcher)
            }
            _ => LinkCompletions::open(candidates, &typed.typed, &source, matcher),
        }
        .map(List::Links)
    }

    /// La lista de `#`. `aliases` y `parents` solo nombran tags en una
    /// página de tag.
    fn tag_list(&mut self, typed: &TypedTag, cx: &App) -> Option<List> {
        let tags = self.session.read(cx).tags();
        let note = self.note_path(cx);
        if matches!(typed.place, TagPlace::Aliases | TagPlace::Parents)
            && !note.as_ref().is_some_and(|path| tags.is_page(path))
        {
            return None;
        }
        let only_here = note
            .as_ref()
            .is_some_and(|note| tags.is_only_once_in(&TagKey::new(&typed.typed), note));
        let candidates = self.completion.tag_candidates(tags);
        let matcher = &mut self.completion.names;
        match self.completion.list.take() {
            Some(List::Tags(list)) if list.is_for(&candidates, &typed.typed, typed.place) => {
                Some(list)
            }
            Some(List::Tags(list)) if list.is_typing(&typed.typed, typed.place) => {
                list.with_candidates(candidates, only_here, matcher)
            }
            _ => TagCompletions::open(candidates, &typed.typed, typed.place, only_here, matcher),
        }
        .map(List::Tags)
    }

    /// Escribe lo elegido y cierra la lista: lo que queda escrito ya es el
    /// tag o el link entero.
    pub(super) fn accept_completion(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(list) = self.completion.list.take() else {
            return;
        };
        // Como una tecla del motor: lo que componía el IME ya no aplica.
        self.marked = None;
        let result = match list {
            List::Tags(list) => {
                let text = list.accept();
                self.update_workspace(cx, |workspace, pane, cx| {
                    workspace.complete_tag(pane, &text, &mut SystemClipboard(cx))
                })
            }
            List::Links(list) => {
                if let Some(target) = self.wikilink_target(list.accept(), cx) {
                    self.update_workspace(cx, |workspace, pane, cx| {
                        workspace.complete_link(pane, &target, &mut SystemClipboard(cx))
                    })
                } else {
                    let message = Message::unnameable(list.accept().as_str());
                    self.update_workspace(cx, |workspace, _, _| workspace.set_message(message));
                    Ok(())
                }
            }
        };
        if let Err(error) = result {
            self.report(&error, cx);
        }
        self.synced_top = None;
        window.refresh();
        cx.notify();
    }

    /// Con qué nombre llega a `target` un wikilink escrito en esta nota.
    fn wikilink_target(&mut self, target: &VaultPath, cx: &App) -> Option<String> {
        let source = self.note_path(cx)?;
        let links = self.session.read(cx).vault().links();
        links.wikilink_target(&source, target)
    }

    pub(super) fn select_next_completion(&mut self, cx: &mut Context<Self>) {
        if let Some(list) = &mut self.completion.list {
            list.selection_mut().select_next();
            cx.notify();
        }
    }

    pub(super) fn select_previous_completion(&mut self, cx: &mut Context<Self>) {
        if let Some(list) = &mut self.completion.list {
            list.selection_mut().select_previous();
            cx.notify();
        }
    }

    /// La lista bajo lo escrito, o encima si abajo no cabe en el texto del
    /// panel (ver [`MenuMetrics::place`]), sobre `layout`: el del frame que
    /// se está pintando. No pasa de `max_width`, el ancho de la ventana.
    pub(super) fn completion_overlay(
        &mut self,
        layout: &FrameLayout,
        max_width: Pixels,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let list = self.completion.list.as_ref()?;
        let wanted = usize::from(theme.layout.completion_rows).min(list.len());
        let typed = list.typed().len();
        let view = self.view(cx)?;
        let (laid, column) = cursor_position(&view, layout)?;
        let x = laid.origin.x + laid.x_for_index(column - typed.min(column));
        let line = Bounds::new(laid.origin, size(laid.width(), laid.height));
        let placement = MenuMetrics {
            row_height: theme.menu_row_height,
            frame: theme.layout.menu_padding + theme.stroke.border,
        }
        .place(wanted, line, layout.viewport.bounds);
        let list = self.completion.list.as_mut()?;
        let visible = list.selection_mut().visible(placement.rows);
        let rows = match &*list {
            List::Tags(list) => rows::tags(list, visible.clone(), theme),
            List::Links(list) => rows::links(list, visible.clone(), theme),
        };
        let accept_keys = AppKeymap::get(cx)
            .keys_for(EditorAction::AcceptCompletion.into(), |view| {
                view == View::Completion
            });
        let menu = CompletionMenu {
            rows,
            visible,
            selected: list.selection().selected(),
            accept_keys,
            max_width,
        }
        .render(theme, cx, |row, element, cx| {
            element
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |editor, _, window, cx| {
                    if let Some(list) = &mut editor.completion.list {
                        list.selection_mut().select(row);
                    }
                    // El foco pudo irse (al panel lateral) con la lista abierta.
                    editor.take_focus(window, cx);
                    editor.accept_completion(window, cx);
                }))
        });
        Some(
            anchored()
                .position(Point { x, y: placement.y })
                .snap_to_window()
                .child(menu)
                .into_any_element(),
        )
    }
}

/// Lo que se está escribiendo.
enum Typed {
    Link(TypedLink),
    Tag(TypedTag),
}

impl Typed {
    fn in_pane(workspace: &Workspace, pane: PaneId) -> Result<Option<Self>, WorkspaceError> {
        if let Some(link) = workspace.typed_link(pane)? {
            return Ok(Some(Self::Link(link)));
        }
        Ok(workspace.typed_tag(pane)?.map(Self::Tag))
    }
}
