//! Cómo se ve la vista de tags del panel (05 §3): cada tag con su ícono,
//! su forma establecida y su conteo; sus notas debajo, y el buzón al final.

use std::ops::Range;

use gpui_kit::{
    AnyElement, Context, Div, IntoElement, ParentElement as _, Styled as _, div, uniform_list,
};
use metalmind_vault::tags::TagIcon;
use metalmind_vault::{EntryKind, VaultPath};

use super::render::row_frame;
use super::{Sidebar, icons};
use crate::panel::{Outline as _, RowMarks};
use crate::tag_outline::{Row, RowKind};
use crate::theme::Theme;

impl Sidebar {
    pub(super) fn tag_body(
        &self,
        theme: &Theme,
        focused: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let active = self.active(cx);
        let theme = theme.clone();
        uniform_list(
            "tag-tree",
            self.panel.tags.rows().len(),
            cx.processor(move |sidebar, range: Range<usize>, _, cx| {
                sidebar.tag_rows(range, focused, active.as_ref(), &theme, cx)
            }),
        )
        .track_scroll(&self.scroll)
        .flex_1()
        .into_any_element()
    }

    fn tag_rows(
        &self,
        range: Range<usize>,
        focused: bool,
        active: Option<&VaultPath>,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let selected = self.panel.tags.selected_row();
        self.panel.tags.rows()[range.clone()]
            .iter()
            .zip(range)
            .map(|(row, at)| {
                let is_active = row.note().is_some_and(|note| active == Some(note));
                let marks = RowMarks::of(selected == Some(at), focused, is_active);
                tag_row(row, at, marks, theme, cx)
            })
            .collect()
    }
}

fn tag_row(
    row: &Row,
    at: usize,
    marks: RowMarks,
    theme: &Theme,
    cx: &mut Context<Sidebar>,
) -> AnyElement {
    let frame = row_frame(at, row.depth, marks, theme, cx).text_color(theme.ui.panel_foreground);
    match &row.kind {
        RowKind::Tag {
            name,
            count,
            icon,
            expandable,
            expanded,
            ..
        } => frame
            .child(icons::arrow(expandable.then_some(*expanded), theme))
            .child(tag_icon(icon.as_ref(), theme))
            .child(label(name))
            .child(count_label(*count, theme)),
        RowKind::Note => {
            let name = row.note().map_or("", VaultPath::name);
            frame
                .child(icons::arrow(None, theme))
                .child(icons::entry(EntryKind::Note, theme))
                .child(label(name))
        }
        RowKind::Inbox { count, expanded } => frame
            .child(icons::arrow((*count > 0).then_some(*expanded), theme))
            .child(glyph("∅", theme))
            .child(label("Sin tags"))
            .child(count_label(*count, theme)),
    }
    .into_any_element()
}

/// El emoji de la página del tag o, si no tiene, un `#` tenue. Los íconos
/// de un set llegan con los de Tabler (#41).
fn tag_icon(icon: Option<&TagIcon>, theme: &Theme) -> Div {
    match icon {
        Some(TagIcon::Emoji(emoji)) => icon_slot(theme).child(emoji.clone()),
        Some(TagIcon::Set { .. }) | None => glyph("#", theme),
    }
}

/// Un carácter tenue en el lugar del ícono.
fn glyph(text: &'static str, theme: &Theme) -> Div {
    icon_slot(theme)
        .text_color(theme.ui.panel_muted)
        .child(text)
}

fn icon_slot(theme: &Theme) -> Div {
    div().w(theme.icon_size).flex_none().flex().justify_center()
}

fn label(text: &str) -> Div {
    div().flex_1().min_w_0().truncate().child(text.to_owned())
}

fn count_label(count: usize, theme: &Theme) -> Div {
    div()
        .flex_none()
        .text_size(theme.ui_small_font_size)
        .text_color(theme.ui.panel_muted)
        .child(count.to_string())
}
