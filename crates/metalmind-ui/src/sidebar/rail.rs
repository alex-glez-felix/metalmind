//! La barra angosta a la izquierda de la ventana, con un botón por vista
//! del panel lateral y otro para la lista de notas: el ratón llega a lo
//! mismo que `space e`, `space t`, `space b` y `space l`.

use gpui_kit::{
    AnyElement, Div, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _,
};
use metalmind_keymap::Command;

use crate::actions::Dispatch;
use crate::icons::{self, Icon};
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
            Icon::Folder,
            Command::ToggleFolders,
            theme,
        ))
        .child(rail_button(
            "rail-tags",
            shown == Some(PanelView::Tags),
            Icon::Tags,
            Command::ToggleTags,
            theme,
        ))
        .child(rail_button(
            "rail-backlinks",
            shown == Some(PanelView::Backlinks),
            Icon::Backlinks,
            Command::ToggleBacklinks,
            theme,
        ))
        .child(rail_button(
            "rail-list",
            list,
            Icon::NoteList,
            Command::ToggleNoteList,
            theme,
        ))
}

fn rail_button(
    id: &'static str,
    active: bool,
    icon: Icon,
    command: Command,
    theme: &Theme,
) -> AnyElement {
    let color = if active {
        theme.ui.panel_foreground
    } else {
        theme.ui.panel_muted
    };
    div()
        .id(id)
        .size(theme.layout.rail_button_size)
        .flex()
        .items_center()
        .justify_center()
        .rounded(theme.radius.md)
        .when(active, |button| button.bg(theme.ui.panel_active))
        .hover(|button| button.bg(theme.ui.panel_hover))
        .on_click(move |_, window, cx| {
            window.dispatch_action(Box::new(Dispatch(command.into())), cx);
        })
        .child(icons::icon(icon, color, theme))
        .into_any_element()
}
