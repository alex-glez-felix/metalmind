//! Cómo se ve un picker: el input con su cursor y la lista virtual, con
//! las letras que coinciden resaltadas solo en las filas visibles.

use std::ops::Range;

use gpui_kit::{
    Context, ElementInputHandler, FontWeight, HighlightStyle, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    StyledText, Window, canvas, div, prelude::FluentBuilder as _, relative, uniform_list,
};

use super::{PickerEvent, PickerItem, PickerView};
use crate::picker::highlight::match_ranges;
use crate::theme::Theme;

impl<T: PickerItem> Render for PickerView<T> {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        let row_height = theme.font_size * theme.line_height + theme.picker_padding_y * 2.;
        let rows = u8::try_from(self.state.len()).unwrap_or(u8::MAX);
        let visible = rows.clamp(1, theme.picker_rows);
        div()
            .key_context(self.key_context())
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::select_previous))
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::confirm_split_right))
            .on_action(cx.listener(Self::confirm_split_down))
            .on_action(cx.listener(Self::dismiss))
            .on_action(cx.listener(Self::delete_backward))
            .on_action(cx.listener(Self::delete_forward))
            .on_action(cx.listener(Self::delete_word_backward))
            .on_action(cx.listener(Self::delete_to_start))
            .on_action(cx.listener(Self::move_left))
            .on_action(cx.listener(Self::move_right))
            .on_action(cx.listener(Self::move_to_start))
            .on_action(cx.listener(Self::move_to_end))
            .on_mouse_down_out(cx.listener(|_, _, _, cx| cx.emit(PickerEvent::Dismissed)))
            .w(theme.picker_width)
            .max_w(relative(0.9))
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(theme.picker_background)
            .border(theme.border_width)
            .border_color(theme.picker_border)
            .rounded(theme.picker_radius)
            .child(self.render_input(&theme, cx))
            .child(
                div()
                    .h(row_height * f32::from(visible))
                    .border_t(theme.border_width)
                    .border_color(theme.picker_border)
                    .child(self.render_list(&theme, row_height, cx)),
            )
    }
}

impl<T: PickerItem> PickerView<T> {
    fn render_input(&self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let line = self.state.line();
        let marked = self.marked.clone().unwrap_or_default();
        let (before, after) = line.text().split_at(line.cursor());
        let empty = line.text().is_empty() && marked.is_empty();
        let entity = cx.entity();
        let focus = self.focus_handle.clone();
        let caret_height = theme.font_size * theme.line_height;
        div()
            .relative()
            .flex()
            .items_center()
            .px(theme.picker_padding_x)
            .py(theme.picker_padding_y * 2.)
            .child(div().child(before.to_owned()))
            .when(!marked.is_empty(), |input| {
                input.child(div().underline().child(marked))
            })
            .child(div().w(theme.caret_width).h(caret_height).bg(theme.caret))
            .when(empty, |input| {
                input.child(
                    div()
                        .text_color(theme.picker_muted)
                        .child(self.options.placeholder.clone()),
                )
            })
            .child(div().child(after.to_owned()))
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
        if self.state.is_empty() {
            let message = if self.state.items().is_empty() {
                self.options.empty.clone()
            } else {
                SharedString::new_static("Sin resultados")
            };
            return div()
                .h(row_height)
                .px(theme.picker_padding_x)
                .flex()
                .items_center()
                .text_color(theme.picker_muted)
                .child(message)
                .into_any_element();
        }
        let theme = theme.clone();
        uniform_list(
            "picker-rows",
            self.state.len(),
            cx.processor(move |this, rows: Range<usize>, _window, cx| {
                rows.map(|row| this.render_row(row, &theme, cx).into_any_element())
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
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let positions = self.state.positions(row, &mut self.paint_matcher);
        let item = self.state.item(row);
        let text = item
            .map(|item| item.as_ref().to_owned())
            .unwrap_or_default();
        let detail = item.and_then(|item| item.detail()).map(str::to_owned);
        let highlight = HighlightStyle {
            color: Some(theme.picker_match),
            font_weight: Some(FontWeight::BOLD),
            ..HighlightStyle::default()
        };
        let highlights = match_ranges(&text, &positions)
            .into_iter()
            .map(|range| (range, highlight));
        let label = StyledText::new(text).with_highlights(highlights);
        let selected = row == self.state.selected();
        let hover = theme.picker_hover;
        div()
            .id(row)
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .gap(theme.status_gap)
            .px(theme.picker_padding_x)
            .py(theme.picker_padding_y)
            .when(selected, |row| row.bg(theme.picker_selected))
            .when(!selected, |row| row.hover(move |style| style.bg(hover)))
            .on_click(cx.listener(move |this, _, _, cx| this.click(row, cx)))
            .child(div().min_w_0().overflow_hidden().child(label))
            .children(detail.map(|detail| {
                div()
                    .flex_none()
                    .text_color(theme.picker_muted)
                    .child(detail)
            }))
    }
}
