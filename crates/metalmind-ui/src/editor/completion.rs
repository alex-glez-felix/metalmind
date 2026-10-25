//! La lista del autocompletado de `#` en el editor (05 §3): se abre al
//! escribir un tag en Insert, se pinta bajo él (o encima, si no cabe) y
//! `tab` o un clic cambian lo escrito por la forma establecida.

use std::sync::Arc;

use gpui_kit::{
    AnyElement, App, Context, Div, InteractiveElement as _, IntoElement, KeyContext, MouseButton,
    ParentElement as _, Point, StatefulInteractiveElement as _, Styled as _, Window, anchored,
    deferred, div, prelude::FluentBuilder as _,
};
use metalmind_keymap::{EditorAction, View, context};
use metalmind_search::FuzzyMatcher;
use metalmind_syntax::TagPlace;
use metalmind_vault::tags::{TagCompletion, TagIcon, Tags};

use super::Editor;
use super::cursor::cursor_position;
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

    /// Tras lo tecleado: abre, filtra o cierra la lista según el tag que se
    /// está escribiendo. `aliases` y `parents` solo nombran tags en una
    /// página de tag.
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
        if matches!(typed.place, TagPlace::Aliases | TagPlace::Parents)
            && !self.note_path(cx).is_some_and(|path| tags.is_page(&path))
        {
            self.completion.close();
            return;
        }
        let candidates = self.completion.candidates(tags);
        self.completion.list = TagCompletions::open(
            candidates,
            &typed.typed,
            typed.place,
            &mut self.completion.matcher,
        );
    }

    /// Cambia lo escrito por el tag elegido y cierra la lista: lo que queda
    /// escrito ya es el tag entero.
    pub(super) fn accept_completion(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(list) = self.completion.list.take() else {
            return;
        };
        let accepted = list.accept();
        let result = self.update_workspace(cx, |workspace, pane, cx| {
            workspace.complete_tag(
                pane,
                &accepted.typed,
                &accepted.text,
                &mut SystemClipboard(cx),
            )
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
    /// texto del panel.
    pub(super) fn completion_overlay(
        &self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let list = self.completion.list.as_ref()?;
        let view = self.view(cx)?;
        let layout = self.layout.as_ref()?;
        let (laid, column) = cursor_position(&view, layout)?;
        let typed = list.typed().len().min(column);
        let x = laid.origin.x + laid.x_for_index(column - typed);
        let rows = u8::try_from(list.rows().len())
            .unwrap_or(u8::MAX)
            .min(theme.layout.completion_rows);
        // El relleno y el borde, arriba y abajo.
        let frame = theme.layout.menu_padding + theme.stroke.border;
        let height = theme.menu_row_height * f32::from(rows) + frame + frame;
        let text_area = layout.viewport.bounds;
        let below = laid.origin.y + laid.height;
        let above = laid.origin.y - height;
        let y = if below + height > text_area.bottom() && above >= text_area.top() {
            above
        } else {
            below
        };
        let rows = usize::from(rows);
        let first = list.selected().saturating_sub(rows - 1);
        let accept_keys = AppKeymap::get(cx)
            .keys_for(EditorAction::AcceptTagCompletion.into(), |view| {
                view == View::TagCompletion
            });
        let mut menu = div()
            .min_w(theme.layout.menu_width)
            .p(theme.layout.menu_padding)
            .flex()
            .flex_col()
            .bg(theme.ui.menu_background)
            .border(theme.stroke.border)
            .border_color(theme.ui.menu_border)
            .rounded(theme.radius.md)
            .map(|element| theme.ui_text(element));
        for (row, (completion, alias)) in list.rows().enumerate().skip(first).take(rows) {
            let selected = row == list.selected();
            let keys = selected.then(|| accept_keys.clone()).flatten();
            menu = menu.child(completion_row(
                row, completion, alias, keys, selected, theme, cx,
            ));
        }
        Some(
            deferred(
                anchored()
                    .position(Point { x, y })
                    .snap_to_window()
                    .child(menu),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }
}

/// Un tag de la lista: su ícono, su nombre, el alias por el que coincidió,
/// su conteo y, en el elegido, la tecla que lo acepta.
fn completion_row(
    row: usize,
    completion: &TagCompletion,
    alias: Option<&str>,
    keys: Option<String>,
    selected: bool,
    theme: &Theme,
    cx: &mut Context<Editor>,
) -> AnyElement {
    let muted = |text: String| {
        div()
            .flex_none()
            .map(|element| theme.ui_small_text(element))
            .text_color(theme.ui.panel_muted)
            .child(text)
    };
    div()
        .id(("tag-completion", row))
        .h(theme.menu_row_height)
        .px(theme.layout.menu_item_padding_x)
        .flex()
        .items_center()
        .gap(theme.layout.panel_gap)
        .rounded(theme.radius.md)
        .text_color(theme.ui.panel_foreground)
        .when(selected, |element| element.bg(theme.ui.menu_hover))
        .hover(|element| element.bg(theme.ui.menu_hover))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(move |editor, _, window, cx| {
            if let Some(list) = &mut editor.completion.list {
                list.select(row);
            }
            editor.accept_completion(window, cx);
        }))
        .child(icon(completion.icon.as_ref(), theme))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .child(completion.name.clone()),
        )
        .children(alias.map(|alias| muted(format!("alias {alias}"))))
        .child(muted(completion.count.to_string()))
        .children(keys.map(muted))
        .into_any_element()
}

/// El emoji de la página del tag o un `#` tenue, como en el panel.
fn icon(icon: Option<&TagIcon>, theme: &Theme) -> Div {
    let slot = div().w(theme.icon_size).flex_none().flex().justify_center();
    match icon {
        Some(TagIcon::Emoji(emoji)) => slot.child(emoji.clone()),
        Some(TagIcon::Set { .. }) | None => slot.text_color(theme.ui.panel_muted).child("#"),
    }
}
