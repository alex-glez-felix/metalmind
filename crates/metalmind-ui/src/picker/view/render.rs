//! Cómo se ve un picker: el input con su cursor y la lista virtual, con
//! lo que coincide resaltado solo en las filas visibles, y a su derecha la
//! vista previa si el delegado la tiene y cabe en la ventana.

use std::ops::Range;

use gpui_kit::{
    Context, InteractiveElement as _, IntoElement, ParentElement as _, Pixels, Render,
    StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _, px,
    relative, uniform_list,
};

use super::row_line::RowLine;
use super::{PickerDelegate, PickerEvent, PickerView};
use crate::line_field::{FieldLook, LineField};
use crate::picker::{fit, preview_width};
use crate::theme::Theme;

impl<D: PickerDelegate> Render for PickerView<D> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        let row_height = theme.picker_row_height();
        let preview = self.preview_width(&theme, window);
        // Con vista previa, la lista no se encoge con los resultados: el
        // panel de al lado no cambia de alto.
        let rows = if preview.is_some() {
            usize::from(theme.layout.picker_rows)
        } else {
            self.delegate.len()
        };
        let list_height = fit::list_height(
            rows,
            theme.layout.picker_rows,
            f32::from(row_height),
            f32::from(theme.stroke.border),
        );
        div()
            .key_context(self.key_context())
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_dispatch))
            .on_mouse_down_out(cx.listener(|_, _, _, cx| cx.emit(PickerEvent::Dismissed)))
            .w(theme.layout.picker_width + preview.unwrap_or_default())
            .max_w(relative(theme.layout.picker_max_share))
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
                    .flex()
                    .border_t(theme.stroke.border)
                    .border_color(theme.ui.picker_border)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .child(self.render_list(&theme, row_height, cx)),
                    )
                    .children(preview.map(|width| self.render_preview(&theme, width))),
            )
    }
}

impl<D: PickerDelegate> PickerView<D> {
    /// El ancho de la vista previa, si el delegado la tiene y cabe.
    fn preview_width(&self, theme: &Theme, window: &Window) -> Option<Pixels> {
        if !self.delegate.has_preview() {
            return None;
        }
        preview_width(
            f32::from(window.viewport_size().width),
            theme.layout.picker_max_share,
            f32::from(theme.layout.picker_width),
            f32::from(theme.layout.picker_preview_width),
            f32::from(theme.layout.picker_preview_min_width),
        )
        .map(px)
    }

    fn render_input(&self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let look = FieldLook {
            warnings: self.delegate.warnings(),
            placeholder: Some(self.options.placeholder.clone()),
        };
        div()
            .debug_selector(|| "picker-input".to_owned())
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
            .child(LineField::element(
                &self.field,
                self.delegate.line(),
                look,
                theme,
                cx,
            ))
            .children(self.delegate.status().map(|status| {
                div()
                    .pl(theme.layout.picker_gap)
                    .flex_none()
                    .map(|element| theme.ui_small_text(element))
                    .text_color(theme.ui.picker_muted)
                    .child(status)
            }))
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
