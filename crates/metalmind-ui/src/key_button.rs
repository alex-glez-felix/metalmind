//! Los botones de texto de los paneles con su tecla al lado, para que el
//! ratón enseñe el teclado (design.md, 3): la tecla es la de la vista donde
//! está el botón.

use gpui_kit::{
    App, Div, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _,
};
use metalmind_keymap::{Action, View};

use crate::actions::{AppKeymap, Dispatch};
use crate::theme::Theme;

/// Un botón de texto apagado que invoca `action`, con su tecla si tiene.
pub fn key_button(
    id: &'static str,
    label: &'static str,
    action: impl Into<Action>,
    view: View,
    theme: &Theme,
    cx: &App,
) -> impl IntoElement {
    let action: Action = action.into();
    div()
        .id(id)
        .flex()
        .flex_none()
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
        .on_click(move |_, window, cx| {
            window.dispatch_action(Box::new(Dispatch(action.into())), cx);
        })
        .children(key_hint(action, view, theme, cx))
        .child(label)
}

/// La tecla de `action` en `view`, en una cajita, si tiene una.
pub fn key_hint(action: impl Into<Action>, view: View, theme: &Theme, cx: &App) -> Option<Div> {
    let keys = AppKeymap::get(cx).keys_for(action.into(), |shown| shown == view)?;
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
