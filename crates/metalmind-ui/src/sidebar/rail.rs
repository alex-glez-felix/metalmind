//! La barra angosta a la izquierda de la ventana, con un botón por vista
//! del panel lateral: el ratón llega a lo mismo que `space e` y `space t`.

use gpui_kit::{
    Action, AnyElement, Div, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _,
};
use metalmind_vault::EntryKind;

use super::icons;
use crate::actions::{ToggleFolders, ToggleTags};
use crate::panel::PanelView;
use crate::theme::Theme;

/// `shown`: la vista que se ve, si el panel está visible.
pub fn rail(shown: Option<PanelView>, theme: &Theme) -> Div {
    div()
        .w(theme.rail_width)
        .h_full()
        .flex_none()
        .flex()
        .flex_col()
        .items_center()
        .gap(theme.panel_gap)
        .py(theme.panel_padding_y)
        .bg(theme.panel_background)
        .border_r(theme.border_width)
        .border_color(theme.panel_border)
        .child(rail_button(
            "rail-folders",
            shown == Some(PanelView::Folders),
            icons::entry(EntryKind::Folder, theme),
            ToggleFolders,
            theme,
        ))
        .child(rail_button(
            "rail-tags",
            shown == Some(PanelView::Tags),
            div().child("#"),
            ToggleTags,
            theme,
        ))
}

fn rail_button<A: Action + Clone>(
    id: &'static str,
    active: bool,
    icon: Div,
    action: A,
    theme: &Theme,
) -> AnyElement {
    div()
        .id(id)
        .size(theme.rail_button_size)
        .flex()
        .items_center()
        .justify_center()
        .rounded(theme.radius_md)
        .text_color(theme.panel_muted)
        .when(active, |button| {
            button
                .bg(theme.panel_active)
                .text_color(theme.panel_foreground)
        })
        .hover(|button| button.bg(theme.panel_hover))
        .on_click(move |_, window, cx| window.dispatch_action(action.boxed_clone(), cx))
        .child(icon)
        .into_any_element()
}
