//! La lista del autocompletado de `#` en el editor (05 §3): se abre al
//! escribir un tag en Insert, se pinta bajo él (o encima, si no cabe) y
//! `tab` o un clic cambian lo escrito por la forma establecida.

use std::sync::Arc;

use gpui_kit::{
    AnyElement, App, Bounds, Context, InteractiveElement as _, IntoElement, KeyContext,
    MouseButton, ParentElement as _, Pixels, Point, StatefulInteractiveElement as _, Window,
    anchored, size,
};
use metalmind_keymap::{EditorAction, View, context};
use metalmind_search::FuzzyMatcher;
use metalmind_syntax::TagPlace;
use metalmind_vault::tags::{TagCompletion, TagKey, Tags};

use super::Editor;
use super::completion_menu::{CompletionMenu, MenuMetrics};
use super::cursor::cursor_position;
use super::layout::FrameLayout;
use crate::actions::{AppKeymap, EDITOR_CONTEXT};
use crate::clipboard::SystemClipboard;
use crate::tag_completion::TagCompletions;
use crate::theme::Theme;

/// Lo que el editor guarda para el autocompletado.
pub(super) struct Completion {
    list: Option<TagCompletions>,
    /// Los candidatos de unos tags: se arman de nuevo solo si llegan otros.
    candidates: Option<(Arc<Tags>, Arc<[TagCompletion]>)>,
    matcher: FuzzyMatcher,
}

impl Default for Completion {
    fn default() -> Self {
        Self {
            list: None,
            candidates: None,
            matcher: FuzzyMatcher::text(),
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

    fn candidates(&mut self, tags: Arc<Tags>) -> Arc<[TagCompletion]> {
        match &self.candidates {
            Some((cached, candidates)) if Arc::ptr_eq(cached, &tags) => Arc::clone(candidates),
            _ => {
                let candidates: Arc<[TagCompletion]> = tags.completions().into();
                self.candidates = Some((tags, Arc::clone(&candidates)));
                candidates
            }
        }
    }
}

impl Editor {
    /// El del editor y, con la lista abierta, el de sus atajos.
    pub(super) fn key_context(&self) -> KeyContext {
        let mut key_context = KeyContext::new_with_defaults();
        key_context.add(EDITOR_CONTEXT);
        if self.completion.is_open() {
            key_context.add(context::TAG_COMPLETION);
        }
        key_context
    }

    /// Tras cualquier cambio del workspace o del vault, la lista abierta
    /// sigue al tag que se está escribiendo con los tags de ahora, o se
    /// cierra.
    pub(super) fn refresh_completion(&mut self, cx: &App) {
        if self.completion.is_open() {
            self.update_completion(cx);
        }
    }

    /// Tras escribir texto: abre, filtra o cierra la lista según el tag que
    /// se está escribiendo. `aliases` y `parents` solo nombran tags en una
    /// página de tag. Si lo escrito no cambió, el tag que eligió el usuario
    /// sigue elegido.
    pub(super) fn update_completion(&mut self, cx: &App) {
        let typed = self
            .workspace
            .read(cx)
            .typed_tag(self.pane)
            .inspect_err(|error| log::error!("{error}"))
            .ok()
            .flatten();
        let Some(typed) = typed else {
            self.completion.close();
            return;
        };
        let tags = self.session.read(cx).tags();
        let note = self.note_path(cx);
        if matches!(typed.place, TagPlace::Aliases | TagPlace::Parents)
            && !note.as_ref().is_some_and(|path| tags.is_page(path))
        {
            self.completion.close();
            return;
        }
        let only_here = note
            .as_ref()
            .is_some_and(|note| tags.is_only_once_in(&TagKey::new(&typed.typed), note));
        let candidates = self.completion.candidates(tags);
        let matcher = &mut self.completion.matcher;
        self.completion.list = match self.completion.list.take() {
            Some(list) if list.is_for(&candidates, &typed.typed, typed.place) => Some(list),
            Some(list) if list.is_typing(&typed.typed, typed.place) => {
                list.with_candidates(candidates, only_here, matcher)
            }
            _ => TagCompletions::open(candidates, &typed.typed, typed.place, only_here, matcher),
        };
    }

    /// Cambia lo escrito por el tag elegido y cierra la lista: lo que queda
    /// escrito ya es el tag entero.
    pub(super) fn accept_completion(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(list) = self.completion.list.take() else {
            return;
        };
        // Como una tecla del motor: lo que componía el IME ya no aplica.
        self.marked = None;
        let text = list.accept();
        let result = self.update_workspace(cx, |workspace, pane, cx| {
            workspace.complete_tag(pane, &text, &mut SystemClipboard(cx))
        });
        if let Err(error) = result {
            self.report(&error, cx);
        }
        self.synced_top = None;
        window.refresh();
        cx.notify();
    }

    pub(super) fn select_next_completion(&mut self, cx: &mut Context<Self>) {
        if let Some(list) = &mut self.completion.list {
            list.select_next();
            cx.notify();
        }
    }

    pub(super) fn select_previous_completion(&mut self, cx: &mut Context<Self>) {
        if let Some(list) = &mut self.completion.list {
            list.select_previous();
            cx.notify();
        }
    }

    /// La lista bajo el nombre del tag, o encima si abajo no cabe en el
    /// texto del panel (ver [`MenuMetrics::place`]), sobre `layout`: el del
    /// frame que se está pintando. No pasa de `max_width`, el ancho de la
    /// ventana.
    pub(super) fn completion_overlay(
        &mut self,
        layout: &FrameLayout,
        max_width: Pixels,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let list = self.completion.list.as_ref()?;
        let wanted = usize::from(theme.layout.completion_rows).min(list.rows().len());
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
        let visible = self.completion.list.as_mut()?.visible(placement.rows);
        let list = self.completion.list.as_ref()?;
        let accept_keys = AppKeymap::get(cx)
            .keys_for(EditorAction::AcceptTagCompletion.into(), |view| {
                view == View::TagCompletion
            });
        let menu = CompletionMenu {
            list,
            visible,
            accept_keys,
            max_width,
        }
        .render(theme, cx, |row, element, cx| {
            element
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |editor, _, window, cx| {
                    if let Some(list) = &mut editor.completion.list {
                        list.select(row);
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
