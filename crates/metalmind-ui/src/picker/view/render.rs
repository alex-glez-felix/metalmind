//! Cómo se ve un picker: el input con su cursor y la lista virtual, con
//! lo que coincide resaltado solo en las filas visibles.

use std::ops::Range;

use gpui_kit::{
    Context, ElementInputHandler, HighlightStyle, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    StyledText, Window, canvas, div, prelude::FluentBuilder as _, px, relative, uniform_list,
};

use super::row_line::RowLine;
use super::{PickerDelegate, PickerEvent, PickerView};
use crate::picker::fit;
use crate::theme::Theme;

impl<D: PickerDelegate> Render for PickerView<D> {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        let row_height = theme.picker_row_height();
        let list_height = fit::list_height(
            self.delegate.len(),
            theme.layout.picker_rows,
            f32::from(row_height),
            f32::from(theme.stroke.border),
        );
        div()
            .key_context(self.key_context())
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_dispatch))
            .on_mouse_down_out(cx.listener(|_, _, _, cx| cx.emit(PickerEvent::Dismissed)))
            .w(theme.layout.picker_width)
            .max_w(relative(0.9))
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(theme.ui.picker_background)
            .border(theme.stroke.border)
            .border_color(theme.ui.picker_border)
            .rounded(theme.radius.lg)
            .child(self.render_input(&theme, cx))
            .child(
                div()
                    .h(px(list_height))
                    .border_t(theme.stroke.border)
                    .border_color(theme.ui.picker_border)
                    .child(self.render_list(&theme, row_height, cx)),
            )
    }
}

/// Un tramo del texto del input: nunca se parte ni se encoge, así lo que no
/// cabe se corre en vez de encimarse.
fn piece() -> gpui_kit::Div {
    div().flex_none().whitespace_nowrap()
}

impl<D: PickerDelegate> PickerView<D> {
    fn render_input(&self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let line = self.delegate.line();
        let marked = self.marked.clone().unwrap_or_default();
        let (before, after) = line.text().split_at(line.cursor());
        let warnings = self.delegate.warnings();
        let warned = |text: &str, offset: usize| {
            with_warnings(text, offset, &warnings, theme.ui.picker_warning)
        };
        let empty = line.text().is_empty() && marked.is_empty();
        let entity = cx.entity();
        let focus = self.focus_handle.clone();
        let caret_height = theme.ui_line();
        let caret = 1 + usize::from(!marked.is_empty());
        self.input_scroll.scroll_to_item(caret);
        let text = div()
            .id("picker-input-text")
            .debug_selector(|| "picker-input-text".to_owned())
            .flex()
            .flex_1()
            .min_w_0()
            .items_center()
            .overflow_x_scroll()
            .track_scroll(&self.input_scroll)
            .child(piece().child(warned(before, 0)))
            .when(!marked.is_empty(), |text| {
                text.child(piece().underline().child(marked))
            })
            .child(
                div()
                    .debug_selector(|| "picker-caret".to_owned())
                    .flex_none()
                    .w(theme.stroke.caret)
                    .h(caret_height)
                    .bg(theme.cursor.insert),
            )
            .when(empty, |text| {
                text.child(
                    piece()
                        .text_color(theme.ui.picker_muted)
                        .child(self.options.placeholder.clone()),
                )
            })
            .child(piece().child(warned(after, before.len())));
        div()
            .debug_selector(|| "picker-input".to_owned())
            .relative()
            .flex()
            .items_center()
            .px(theme.layout.picker_padding_x)
            .py(theme.layout.picker_input_padding_y)
            .children(self.options.focus.clone().map(|focus| {
                div()
                    .debug_selector(|| "picker-focus".to_owned())
                    .flex_none()
                    .max_w(relative(theme.layout.picker_detail_share))
                    .truncate()
                    .mr(theme.layout.picker_gap)
                    .text_color(theme.ui.focus_indicator)
                    .child(focus)
            }))
            .child(text)
            .children(self.delegate.status().map(|status| {
                div()
                    .pl(theme.layout.picker_gap)
                    .flex_none()
                    .map(|element| theme.ui_small_text(element))
                    .text_color(theme.ui.picker_muted)
                    .child(status)
            }))
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, (), window, cx| {
                        window.handle_input(&focus, ElementInputHandler::new(bounds, entity), cx);
                    },
                )
                .absolute()
                .size_full(),
            )
    }

    fn render_list(
        &self,
        theme: &Theme,
        row_height: Pixels,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        if self.delegate.len() == 0 {
            return div()
                .h(row_height)
                .px(theme.layout.picker_padding_x)
                .flex()
                .items_center()
                .text_color(theme.ui.picker_muted)
                .child(self.delegate.empty())
                .into_any_element();
        }
        let theme = theme.clone();
        uniform_list(
            "picker-rows",
            self.delegate.len(),
            cx.processor(move |this, rows: Range<usize>, _window, cx| {
                rows.map(|row| {
                    this.render_row(row, &theme, row_height, cx)
                        .into_any_element()
                })
                .collect()
            }),
        )
        .track_scroll(&self.scroll)
        .size_full()
        .into_any_element()
    }

    fn render_row(
        &mut self,
        row: usize,
        theme: &Theme,
        row_height: Pixels,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected = row == self.delegate.selected();
        let hover = theme.ui.picker_hover;
        let line = RowLine::new(self.delegate.row(row), theme);
        div()
            .id(row)
            .w_full()
            .h(row_height)
            .flex()
            .items_center()
            .px(theme.layout.picker_padding_x)
            .when(selected, |row| row.bg(theme.ui.picker_selected))
            .when(!selected, |row| row.hover(move |style| style.bg(hover)))
            .on_click(cx.listener(move |this, _, _, cx| this.click(row, cx)))
            .child(line)
    }
}

/// `text`, que empieza en el byte `offset` de lo escrito, con los avisos que
/// caen en él pintados de `color`.
fn with_warnings(text: &str, offset: usize, warnings: &[Range<usize>], color: Hsla) -> StyledText {
    let style = HighlightStyle {
        color: Some(color),
        ..HighlightStyle::default()
    };
    let end = offset + text.len();
    let highlights = warnings
        .iter()
        .map(|warning| warning.start.max(offset)..warning.end.min(end))
        .filter(|range| range.start < range.end)
        .map(|range| (range.start - offset..range.end - offset, style))
        .collect::<Vec<_>>();
    StyledText::new(SharedString::from(text.to_owned())).with_highlights(highlights)
}
