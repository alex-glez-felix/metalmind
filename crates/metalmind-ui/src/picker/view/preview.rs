//! La vista previa a la derecha de la lista: las líneas que caben, con la
//! del resultado marcada y centrada, y lo que coincide resaltado.

use gpui_kit::{
    Div, HighlightStyle, InteractiveElement as _, IntoElement, ParentElement as _, Pixels,
    SharedString, Styled as _, StyledText, div, prelude::FluentBuilder as _,
};

use super::{PickerDelegate, PickerView};
use crate::picker::{Preview, PreviewLine, lines_that_fit, visible_lines};
use crate::theme::Theme;

impl<D: PickerDelegate> PickerView<D> {
    pub(super) fn render_preview(&self, theme: &Theme, width: Pixels) -> impl IntoElement {
        let pane = div()
            .debug_selector(|| "picker-preview".to_owned())
            .flex_none()
            .w(width)
            .h_full()
            .overflow_hidden()
            .py(theme.rows.picker_padding)
            .border_l(theme.stroke.border)
            .border_color(theme.ui.picker_border);
        match self.delegate.preview() {
            None => pane,
            Some(Preview::Message(message)) => pane
                .px(theme.layout.picker_padding_x)
                .text_color(theme.ui.picker_muted)
                .child(message.clone()),
            Some(Preview::Lines { lines, focus }) => {
                let fit = lines_that_fit(
                    f32::from(theme.picker_preview_text_height()),
                    f32::from(theme.ui_line()),
                );
                let visible = visible_lines(lines.len(), *focus, fit);
                let digits = lines[visible.clone()]
                    .iter()
                    .map(|line| line.number.to_string().len())
                    .max()
                    .unwrap_or(1);
                pane.children(
                    visible
                        .map(|index| line_row(&lines[index], Some(index) == *focus, digits, theme)),
                )
            }
        }
    }
}

fn line_row(line: &PreviewLine, focused: bool, digits: usize, theme: &Theme) -> Div {
    let matched = HighlightStyle {
        color: Some(theme.ui.picker_match),
        ..HighlightStyle::default()
    };
    let highlights = line
        .highlights
        .iter()
        .map(|range| (range.clone(), matched))
        .collect::<Vec<_>>();
    div()
        .when(focused, |row| {
            row.debug_selector(|| "picker-preview-focus".to_owned())
        })
        .h(theme.ui_line())
        .flex()
        .items_center()
        .gap(theme.layout.picker_padding_x)
        .px(theme.layout.picker_padding_x)
        .when(focused, |row| row.bg(theme.ui.picker_selected))
        .child(
            div()
                .flex_none()
                .whitespace_nowrap()
                .text_color(theme.ui.picker_muted)
                .child(format!("{:>digits$}", line.number)),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .whitespace_nowrap()
                .overflow_hidden()
                .child(
                    StyledText::new(SharedString::from(line.text.clone()))
                        .with_highlights(highlights),
                ),
        )
}
