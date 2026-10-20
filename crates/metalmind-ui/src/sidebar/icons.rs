//! Íconos del panel. TODO(#41): los de Tabler, como decide 05 §7; mientras
//! tanto, formas simples con los colores del tema.

use gpui_kit::{Div, Hsla, ParentElement as _, Styled as _, div};
use metalmind_vault::EntryKind;

use crate::theme::Theme;

/// Una carpeta llena, una nota con borde y un adjunto con borde tenue.
pub fn entry(kind: EntryKind, theme: &Theme) -> Div {
    let size = theme.icon_size;
    let slot = div()
        .size(size)
        .flex_none()
        .flex()
        .items_center()
        .justify_center();
    let shape = match kind {
        EntryKind::Folder => div()
            .w(size)
            .h(theme.icon_short_side())
            .rounded(theme.radius.sm)
            .bg(theme.ui.panel_folder),
        EntryKind::Note => page(size, theme.ui.panel_note, theme),
        EntryKind::Attachment => page(size, theme.ui.panel_muted, theme),
    };
    slot.child(shape)
}

fn page(size: gpui_kit::Pixels, color: Hsla, theme: &Theme) -> Div {
    div()
        .w(theme.icon_short_side())
        .h(size)
        .rounded(theme.radius.sm)
        .border(theme.stroke.border)
        .border_color(color)
}

/// `▸` o `▾` delante de una carpeta; un hueco del mismo ancho delante de
/// un archivo, para que los nombres queden alineados.
pub fn disclosure(kind: EntryKind, expanded: bool, theme: &Theme) -> Div {
    let slot = div()
        .w(theme.icon_size)
        .flex_none()
        .text_color(theme.ui.panel_muted);
    match (kind, expanded) {
        (EntryKind::Folder, true) => slot.child("▾"),
        (EntryKind::Folder, false) => slot.child("▸"),
        _ => slot,
    }
}
