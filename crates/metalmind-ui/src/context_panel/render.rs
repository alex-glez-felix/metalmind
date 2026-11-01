//! Cómo se ve el panel: el encabezado con la nota, las dos pestañas con
//! cuántos tiene cada una, las filas de la que se ve y, abajo, qué hace el
//! ratón con su tecla (design.md, 3).

use gpui_kit::{
    Context, Div, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _,
};
use metalmind_keymap::{Command, ContextAction, View, context};
use metalmind_syntax::note_name;

use super::ContextPanel;
use crate::key_button::{key_button, key_hint};
use crate::note_context::ContextTab;
use crate::theme::Theme;

impl Render for ContextPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        let focused = self.focus_handle.contains_focused(window, cx);
        div()
            .id("context-panel")
            .key_context(context::CONTEXT)
            .track_focus(&self.focus_handle)
            .w(theme.layout.context_width)
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .bg(theme.ui.panel_background)
            .text_color(theme.ui.panel_foreground)
            .border_l(theme.stroke.border)
            .border_color(theme.ui.panel_border)
            .on_action(cx.listener(Self::on_dispatch))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|panel, _, window, cx| window.focus(&panel.focus_handle, cx)),
            )
            .child(self.header(&theme, cx))
            .child(self.tabs(&theme, cx))
            .child(self.body(&theme, focused, cx))
            .child(footer(&theme, cx))
    }
}

impl ContextPanel {
    /// «Contexto» y la nota de la que es, y cómo se cierra.
    fn header(&self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let note = self.context.note().map(|note| {
            note_name::strip_extension(note.name())
                .unwrap_or(note.name())
                .to_owned()
        });
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(theme.layout.panel_gap)
            .h(theme.panel_row_height)
            .px(theme.layout.panel_padding_x)
            .border_b(theme.stroke.border)
            .border_color(theme.ui.panel_border)
            .child(
                div()
                    .flex_none()
                    .font_weight(theme.text.strong)
                    .child("Contexto"),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(theme.ui.panel_muted)
                    .children(note),
            )
            .child(key_button(
                "context-close",
                "cerrar",
                Command::ToggleContext,
                View::Context,
                theme,
                cx,
            ))
    }

    /// Enlazan aquí y salientes, con cuántos tiene cada una; un clic cambia.
    fn tabs(&self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let (backlinks, outgoing) = self.context.counts();
        let shown = self.context.tab();
        let tab =
            |id, tab: ContextTab, label: &'static str, count: usize, cx: &mut Context<Self>| {
                let on = shown == tab;
                div()
                    .id(id)
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(theme.layout.panel_button_padding_x)
                    .h_full()
                    .px(theme.layout.panel_button_padding_x)
                    .border_b(theme.stroke.tab)
                    .border_color(if on {
                        theme.ui.panel_tab
                    } else {
                        gpui_kit::transparent_black()
                    })
                    .text_color(if on {
                        theme.ui.panel_foreground
                    } else {
                        theme.ui.panel_muted
                    })
                    .hover(|tab| tab.text_color(theme.ui.panel_foreground))
                    .on_click(cx.listener(move |panel, _, window, cx| {
                        panel.set_tab(tab, window, cx);
                    }))
                    .child(label)
                    .child(muted(theme).child(count.to_string()))
            };
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(theme.layout.panel_gap)
            .h(theme.panel_row_height)
            .px(theme.layout.panel_padding_x)
            .border_b(theme.stroke.border)
            .border_color(theme.ui.panel_border)
            .child(tab(
                "context-tab-backlinks",
                ContextTab::Backlinks,
                "Enlazan aquí",
                backlinks,
                cx,
            ))
            .child(tab(
                "context-tab-outgoing",
                ContextTab::Outgoing,
                "Salientes",
                outgoing,
                cx,
            ))
            .child(div().flex_1())
            .children(key_hint(ContextAction::NextTab, View::Context, theme, cx))
    }
}

/// Abrir es un clic en la fila; al lado y abajo, estos.
fn footer(theme: &Theme, cx: &mut Context<ContextPanel>) -> Div {
    let buttons: [(&'static str, &'static str, ContextAction); 2] = [
        (
            "context-split-right",
            "al lado",
            ContextAction::OpenSplitRight,
        ),
        ("context-split-down", "abajo", ContextAction::OpenSplitDown),
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
        .children(
            buttons
                .map(|(id, label, action)| key_button(id, label, action, View::Context, theme, cx)),
        )
}

fn muted(theme: &Theme) -> Div {
    theme.ui_small_text(div().text_color(theme.ui.panel_muted))
}
