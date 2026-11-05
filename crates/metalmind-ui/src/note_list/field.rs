//! El campo bajo la cabecera de la lista: el filtro, que se escribe en su
//! lugar, o los tags que se le agregan a la nota elegida.

use gpui_kit::{
    Context, Div, InteractiveElement as _, ParentElement as _, StatefulInteractiveElement as _,
    Styled as _, div,
};
use metalmind_keymap::{ListAction, View};

use super::model::Prompt;
use super::render::dispatch;
use super::view::NoteListView;
use crate::key_button::key_hint;
use crate::line_field::{FieldLook, LineField, field_box, warned_text};
use crate::theme::Theme;

impl NoteListView {
    /// El filtro, o los tags que se le agregan a la nota elegida.
    pub(super) fn field(&self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let frame = div()
            .flex()
            .flex_col()
            .flex_none()
            .gap(theme.layout.panel_gap)
            .px(theme.layout.panel_padding_x)
            .pb(theme.layout.panel_padding_y)
            .border_b(theme.stroke.border)
            .border_color(theme.ui.panel_border);
        if self.list.prompt() == Some(Prompt::Tag) {
            let title = self
                .list
                .tag_target_card()
                .map_or_else(String::new, |card| card.title.to_owned());
            return frame
                .child(
                    div()
                        .text_color(theme.ui.panel_muted)
                        .truncate()
                        .child(format!("Tags para «{title}»")),
                )
                .child(field_box(theme).child(LineField::element(
                    &self.field,
                    self.list.tag_line(),
                    FieldLook::default(),
                    theme,
                    cx,
                )));
        }
        let editing = self.list.prompt() == Some(Prompt::Filter);
        let line = self.list.filter();
        let warnings = self.list.warnings();
        let content = if editing {
            let look = FieldLook {
                warnings,
                placeholder: None,
            };
            LineField::element(&self.field, line, look, theme, cx)
        } else if line.text().is_empty() {
            div()
                .flex_1()
                .text_color(theme.ui.panel_muted)
                .child("filtrar")
        } else {
            let warning = theme.ui.picker_warning;
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .overflow_hidden()
                .child(warned_text(line.text(), 0, &warnings, warning))
        };
        frame.child(
            field_box(theme)
                .id("list-filter")
                .justify_between()
                .on_click(|_, window, cx| dispatch(ListAction::Filter, window, cx))
                .child(content)
                .children(key_hint(ListAction::Filter, View::NoteList, theme, cx)),
        )
    }
}
