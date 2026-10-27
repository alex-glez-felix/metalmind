//! La barra angosta a la izquierda de la ventana, con un botón por vista
//! del panel lateral y otro para la lista de notas: el ratón llega a lo
//! mismo que `space e`, `space t`, `space b` y `space l`.

use gpui_kit::{
    AnyElement, Div, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _,
};
use metalmind_keymap::Command;
use metalmind_vault::EntryKind;

use super::icons;
use crate::actions::Dispatch;
use crate::panel::PanelView;
use crate::theme::Theme;

/// `shown`: la vista que se ve, si el panel está visible; `list`, si se ve
/// la lista de notas.
pub fn rail(shown: Option<PanelView>, list: bool, theme: &Theme) -> Div {
    div()
        .w(theme.layout.rail_width)
        .h_full()
        .flex_none()
        .flex()
        .flex_col()
        .items_center()
        .gap(theme.layout.panel_gap)
        .py(theme.layout.panel_padding_y)
        .bg(theme.ui.panel_background)
        .border_r(theme.stroke.border)
        .border_color(theme.ui.panel_border)
        .child(rail_button(
            "rail-folders",
            shown == Some(PanelView::Folders),
            icons::entry(EntryKind::Folder, theme),
            Command::ToggleFolders,
            theme,
        ))
        .child(rail_button(
            "rail-tags",
            shown == Some(PanelView::Tags),
            div().child("#"),
            Command::ToggleTags,
            theme,
        ))
        .child(rail_button(
            "rail-backlinks",
            shown == Some(PanelView::Backlinks),
            div().child("↩"),
            Command::ToggleBacklinks,
            theme,
        ))
        .child(rail_button(
            "rail-list",
            list,
            div().child("≡"),
            Command::ToggleNoteList,
            theme,
        ))
}

fn rail_button(
    id: &'static str,
    active: bool,
    icon: Div,
    command: Command,
    theme: &Theme,
) -> AnyElement {
    div()
        .id(id)
        .size(theme.layout.rail_button_size)
        .flex()
        .items_center()
        .justify_center()
        .rounded(theme.radius.md)
        .text_color(theme.ui.panel_muted)
        .when(active, |button| {
            button
                .bg(theme.ui.panel_active)
                .text_color(theme.ui.panel_foreground)
        })
        .hover(|button| button.bg(theme.ui.panel_hover))
        .on_click(move |_, window, cx| {
            window.dispatch_action(Box::new(Dispatch(command.into())), cx);
        })
        .child(icon)
        .into_any_element()
}
