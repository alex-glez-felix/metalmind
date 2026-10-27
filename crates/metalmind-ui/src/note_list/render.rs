//! Cómo se ve la lista: la cabecera con la fuente y cuántas notas trae, el
//! campo del filtro (`field.rs`), las filas en una `uniform_list` (solo se pintan las visibles,
//! tenga la fuente las notas que tenga) y las teclas de lo que se hace con
//! el ratón.

use std::ops::Range;

use gpui_kit::{
    AnyElement, ClickEvent, Context, Div, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _, uniform_list,
};
use jiff::Zoned;
use metalmind_keymap::{Action, Command, ListAction, View};
use metalmind_vault::VaultPath;
use metalmind_vault::tags::TagKey;

use super::cards::{Card, stem};
use super::date::when;
use super::view::NoteListView;
use crate::actions::{AppKeymap, Dispatch};
use crate::panel::{Fill, RowMarks};
use crate::theme::Theme;

impl Render for NoteListView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        let focused = self.focus_handle.contains_focused(window, cx);
        div()
            .id("note-list")
            .key_context(self.key_context())
            .track_focus(&self.focus_handle)
            .w(theme.layout.list_width)
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .bg(theme.ui.panel_background)
            .text_color(theme.ui.panel_foreground)
            .border_r(theme.stroke.border)
            .border_color(theme.ui.panel_border)
            .on_action(cx.listener(Self::on_dispatch))
            .on_key_down(cx.listener(Self::on_key_down))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _, window, cx| window.focus(&view.focus_handle, cx)),
            )
            .child(self.header(&theme, cx))
            .child(self.field(&theme, cx))
            .child(self.body(&theme, focused, cx))
            .child(footer(&theme, cx))
    }
}

impl NoteListView {
    /// La fuente, que se cambia con un clic, y cuántas notas hay.
    fn header(&self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let (shown, total) = (self.list.len(), self.list.total());
        let count = match (shown == total, total) {
            (true, 1) => "1 nota".to_owned(),
            (true, _) => format!("{total} notas"),
            (false, _) => format!("{shown} de {total}"),
        };
        let source = div()
            .id("list-source")
            .min_w_0()
            .flex()
            .items_center()
            .gap(theme.layout.panel_gap)
            .px(theme.layout.panel_button_padding_x)
            .rounded(theme.radius.md)
            .hover(|source| source.bg(theme.ui.panel_hover))
            .on_click(|_, window, cx| dispatch(Command::ChooseListSource, window, cx))
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .font_weight(theme.text.strong)
                    .child(self.list.source().label()),
            )
            .children(key_hint(Command::ChooseListSource, theme, cx));
        div()
            .flex()
            .flex_none()
            .justify_between()
            .items_center()
            .gap(theme.layout.panel_gap)
            .h(theme.panel_row_height)
            .px(theme.layout.panel_padding_x)
            .child(source)
            .child(
                div()
                    .flex_none()
                    .text_color(theme.ui.panel_muted)
                    .map(|count| theme.ui_small_text(count))
                    .child(count),
            )
    }

    fn body(&self, theme: &Theme, focused: bool, cx: &mut Context<Self>) -> AnyElement {
        if self.list.is_empty() {
            // Mientras llegan las tarjetas, nada: sin spinners (design.md).
            let text = if !self.list.is_current() {
                ""
            } else if self.list.total() == 0 {
                "Ninguna nota."
            } else {
                "Nada coincide con el filtro."
            };
            return div()
                .flex_1()
                .px(theme.layout.panel_padding_x)
                .py(theme.layout.panel_padding_y)
                .text_color(theme.ui.panel_muted)
                .child(text)
                .into_any_element();
        }
        let active = self.active(cx);
        let now = Zoned::now();
        let theme = theme.clone();
        uniform_list(
            "note-list-rows",
            self.list.len(),
            cx.processor(move |view, range: Range<usize>, _, cx| {
                view.rows(range, focused, active.as_ref(), &now, &theme, cx)
            }),
        )
        .track_scroll(&self.scroll)
        .flex_1()
        .into_any_element()
    }

    fn rows(
        &self,
        range: Range<usize>,
        focused: bool,
        active: Option<&VaultPath>,
        now: &Zoned,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let selected = self.list.selected_row();
        range
            .filter_map(|row| {
                let note = self.list.note(row)?;
                let marks = RowMarks::of(selected == Some(row), focused, active == Some(note));
                let tags = self.tag_names(note);
                Some(row_view(
                    row,
                    note,
                    self.list.card(row),
                    &tags,
                    marks,
                    now,
                    theme,
                    cx,
                ))
            })
            .collect()
    }

    /// Los tags de la nota, cada uno una vez y en su forma establecida.
    fn tag_names(&self, note: &VaultPath) -> Vec<String> {
        let tags = self.list.tags();
        let mut seen: Vec<&TagKey> = Vec::new();
        let mut names = Vec::new();
        for occurrence in tags
            .index()
            .note(note)
            .map_or(&[][..], |note| &note.occurrences)
        {
            if seen.contains(&&occurrence.key) {
                continue;
            }
            seen.push(&occurrence.key);
            let name = tags
                .display_name(&occurrence.key)
                .unwrap_or(&occurrence.spelling);
            names.push(format!("#{name}"));
        }
        names
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "una fila es todo lo que la pinta"
)]
fn row_view(
    row: usize,
    note: &VaultPath,
    card: Option<&Card>,
    tags: &[String],
    marks: RowMarks,
    now: &Zoned,
    theme: &Theme,
    cx: &mut Context<NoteListView>,
) -> AnyElement {
    let title = card.map_or_else(|| stem(note).to_owned(), |card| card.title.clone());
    let date = card
        .and_then(|card| card.modified)
        .map(|modified| when(modified, now))
        .unwrap_or_default();
    let excerpt = card.map(|card| card.excerpt.clone()).unwrap_or_default();
    let folder = note
        .parent()
        .filter(|parent| !parent.is_root())
        .map(|parent| format!("{parent}/"))
        .unwrap_or_default();
    let small_line = theme.ui_small_font_size * theme.text.ui_sm.line_height;
    let excerpt_lines = f32::from(theme.layout.list_excerpt_lines);
    // GPUI da un solo color de borde por elemento: el separador va en la
    // fila y el contorno de la elegida sin foco, en lo de adentro.
    let card = div()
        .size_full()
        .flex()
        .flex_col()
        .gap(theme.layout.list_row_gap)
        .px(theme.layout.panel_padding_x)
        .py(theme.layout.list_row_padding_y)
        .overflow_hidden()
        .border_l(theme.stroke.caret)
        .border_color(if marks.outline {
            theme.ui.panel_selection_unfocused
        } else {
            gpui_kit::transparent_black()
        });
    let content = card
        .child(
            div()
                .flex()
                .justify_between()
                .items_baseline()
                .gap(theme.layout.panel_gap)
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .font_weight(theme.text.strong)
                        .child(title),
                )
                .child(small(theme).flex_none().child(date)),
        )
        .child(
            small(theme)
                .h(small_line * excerpt_lines)
                .overflow_hidden()
                .line_clamp(usize::from(theme.layout.list_excerpt_lines))
                .text_ellipsis()
                .child(excerpt),
        )
        .child(
            div()
                .flex()
                .justify_between()
                .gap(theme.layout.panel_gap)
                .map(|line| theme.ui_small_text(line))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(theme.syntax.tag)
                        .child(tags.join(" ")),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .max_w_1_2()
                        .min_w_0()
                        .truncate()
                        .text_color(theme.ui.panel_muted)
                        .child(folder),
                ),
        );
    div()
        .id(("list-row", row))
        .h(theme.list_row_height())
        .w_full()
        .border_b(theme.stroke.border)
        .border_color(theme.ui.panel_border)
        .map(|element| match marks.fill {
            Some(Fill::Selection) => element.bg(theme.ui.panel_selection),
            Some(Fill::Active) => element.bg(theme.ui.panel_active),
            None => element.hover(|element| element.bg(theme.ui.panel_hover)),
        })
        .on_click(cx.listener(move |view, event: &ClickEvent, window, cx| {
            view.click(row, event.click_count(), window, cx);
        }))
        .child(content)
        .into_any_element()
}

/// Lo secundario de una fila: fecha, extracto y carpeta.
fn small(theme: &Theme) -> Div {
    theme.ui_small_text(div().text_color(theme.ui.panel_muted))
}

/// Qué hace el ratón en la lista, con su tecla: lo enseña (design.md, 3).
/// Abrir es un clic en la fila.
fn footer(theme: &Theme, cx: &mut Context<NoteListView>) -> Div {
    let buttons: [(&'static str, &'static str, Action); 2] = [
        ("list-tag", "etiquetar", ListAction::Tag.into()),
        ("list-fold", "plegar", Command::ToggleNoteList.into()),
    ];
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(theme.layout.panel_gap)
        .h(theme.panel_row_height)
        .px(theme.layout.panel_padding_x)
        .border_t(theme.stroke.border)
        .border_color(theme.ui.panel_border)
        .map(|footer| theme.ui_small_text(footer))
        .children(buttons.map(|(id, label, action)| {
            div()
                .id(id)
                .flex()
                .items_center()
                .gap(theme.layout.panel_button_padding_x)
                .px(theme.layout.panel_button_padding_x)
                .rounded(theme.radius.md)
                .text_color(theme.ui.panel_muted)
                .hover(|button| {
                    button
                        .bg(theme.ui.panel_hover)
                        .text_color(theme.ui.panel_foreground)
                })
                .on_click(move |_, window, cx| dispatch(action, window, cx))
                .children(key_hint(action, theme, cx))
                .child(label)
        }))
}

pub(super) fn dispatch(action: impl Into<Action>, window: &mut Window, cx: &mut gpui_kit::App) {
    let action: Action = action.into();
    window.dispatch_action(Box::new(Dispatch(action.into())), cx);
}

/// La tecla de `action` en la lista, en una cajita, si tiene una.
pub(super) fn key_hint(
    action: impl Into<Action>,
    theme: &Theme,
    cx: &gpui_kit::App,
) -> Option<Div> {
    let keys = AppKeymap::get(cx).keys_for(action.into(), |view| view == View::NoteList)?;
    Some(
        div()
            .flex_none()
            .px(theme.layout.panel_button_padding_x)
            .border(theme.stroke.border)
            .border_color(theme.ui.panel_border)
            .rounded(theme.radius.sm)
            .text_color(theme.ui.panel_muted)
            .map(|hint| theme.ui_small_text(hint))
            .font_weight(theme.text.normal)
            .child(keys),
    )
}
