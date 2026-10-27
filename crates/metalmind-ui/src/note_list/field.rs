//! El campo bajo la cabecera de la lista: el filtro, que se escribe en su
//! lugar, o los tags que se le agregan a la nota elegida.

use std::ops::Range;

use gpui_kit::{
    Context, Div, ElementInputHandler, HighlightStyle, InteractiveElement as _, IntoElement,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, StyledText,
    canvas, div,
};
use metalmind_keymap::ListAction;

use super::model::Prompt;
use super::render::{dispatch, key_hint};
use super::view::NoteListView;
use crate::picker::InputLine;
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
                .map_or_else(String::new, |card| card.title.clone());
            return frame
                .child(
                    div()
                        .text_color(theme.ui.panel_muted)
                        .truncate()
                        .child(format!("Tags para «{title}»")),
                )
                .child(
                    input_box(theme)
                        .relative()
                        .child(line_with_caret(self.list.tag_line(), &[], theme))
                        .child(self.input_handler(cx)),
                );
        }
        let editing = self.list.prompt() == Some(Prompt::Filter);
        let line = self.list.filter();
        let content = if editing || !line.text().is_empty() {
            let warnings = self.list.warnings();
            let shown = if editing {
                line_with_caret(line, &warnings, theme)
            } else {
                div().child(input_text(line.text(), &warnings, theme))
            };
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .overflow_hidden()
                .child(shown)
        } else {
            div()
                .flex_1()
                .text_color(theme.ui.panel_muted)
                .child("filtrar")
        };
        frame.child(
            input_box(theme)
                .id("list-filter")
                .justify_between()
                .on_click(|_, window, cx| dispatch(ListAction::Filter, window, cx))
                .relative()
                .child(content)
                .children(key_hint(ListAction::Filter, theme, cx))
                .children(editing.then(|| self.input_handler(cx))),
        )
    }

    /// El texto, el IME y las teclas muertas llegan al campo por aquí.
    fn input_handler(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (focus, entity) = (self.focus_handle.clone(), cx.entity());
        canvas(
            |_, _, _| (),
            move |bounds, (), window, cx| {
                window.handle_input(&focus, ElementInputHandler::new(bounds, entity), cx);
            },
        )
        .absolute()
        .size_full()
    }
}

fn input_box(theme: &Theme) -> Div {
    div()
        .flex()
        .items_center()
        .gap(theme.layout.panel_gap)
        .h(theme.panel_row_height)
        .px(theme.layout.panel_padding_x)
        .bg(theme.ui.background)
        .border(theme.stroke.border)
        .border_color(theme.ui.panel_border)
        .rounded(theme.radius.md)
        .whitespace_nowrap()
        .overflow_hidden()
}

/// Lo escrito con los tags que no encuentran nada marcados.
fn input_text(text: &str, warnings: &[Range<usize>], theme: &Theme) -> StyledText {
    let style = HighlightStyle {
        color: Some(theme.ui.picker_warning),
        ..HighlightStyle::default()
    };
    let highlights: Vec<(Range<usize>, HighlightStyle)> = warnings
        .iter()
        .filter(|range| range.start < text.len())
        .map(|range| (range.start..range.end.min(text.len()), style))
        .collect();
    StyledText::new(SharedString::from(text.to_owned())).with_highlights(highlights)
}

/// La línea con el cursor de barra, como la línea de comandos.
fn line_with_caret(line: &InputLine, warnings: &[Range<usize>], theme: &Theme) -> Div {
    let (before, after) = line.text().split_at(line.cursor());
    let after_warnings: Vec<Range<usize>> = warnings
        .iter()
        .filter(|range| range.end > before.len())
        .map(|range| range.start.saturating_sub(before.len())..range.end - before.len())
        .collect();
    div()
        .flex()
        .items_center()
        .child(input_text(before, warnings, theme))
        .child(
            div()
                .w(theme.stroke.caret)
                .h(theme.ui_font_size)
                .flex_none()
                .bg(theme.cursor.insert),
        )
        .child(input_text(after, &after_warnings, theme))
}
