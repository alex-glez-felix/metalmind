//! La vista previa en la ventana: el título con el resumen, los avisos del
//! plan, la lista de cambios y los botones de aplicar y cancelar. Flota
//! sobre los paneles como un picker.

use std::ops::Range;

use gpui_kit::{
    AnyElement, App, Context, EventEmitter, FocusHandle, Focusable, Hsla, InteractiveElement as _,
    IntoElement, KeyContext, ParentElement as _, Pixels, Render, ScrollStrategy, SharedString,
    StatefulInteractiveElement as _, Styled, UniformListScrollHandle, Window, div,
    prelude::FluentBuilder as _, px, relative, uniform_list,
};
use metalmind_keymap::{Action, ChangePreviewAction, View, context};

use super::preview::PreviewRow;
use super::{Blocked, ChangePreview};
use crate::actions::{AppKeymap, Dispatch};
use crate::picker::fit;
use crate::theme::Theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangePreviewEvent {
    Apply,
    Cancel,
}

pub struct ChangePreviewView {
    preview: ChangePreview,
    focus_handle: FocusHandle,
    scroll: UniformListScrollHandle,
}

impl EventEmitter<ChangePreviewEvent> for ChangePreviewView {}

impl Focusable for ChangePreviewView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl ChangePreviewView {
    pub fn new(preview: ChangePreview, cx: &mut Context<Self>) -> Self {
        Self {
            preview,
            focus_handle: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(),
        }
    }

    #[must_use]
    pub fn preview(&self) -> &ChangePreview {
        &self.preview
    }

    pub fn preview_mut(&mut self) -> &mut ChangePreview {
        &mut self.preview
    }

    fn on_dispatch(
        &mut self,
        Dispatch(invocation): &Dispatch,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Action::ChangePreview(action) = invocation.action else {
            cx.propagate();
            return;
        };
        match action {
            ChangePreviewAction::SelectNext => self.preview.select_next(),
            ChangePreviewAction::SelectPrevious => self.preview.select_previous(),
            ChangePreviewAction::ToggleChange => self.preview.toggle_selected(),
            ChangePreviewAction::Apply => {
                if self.preview.blocked().is_none() {
                    cx.emit(ChangePreviewEvent::Apply);
                }
            }
            ChangePreviewAction::Cancel => cx.emit(ChangePreviewEvent::Cancel),
        }
        self.changed(cx);
    }

    fn changed(&self, cx: &mut Context<Self>) {
        self.scroll
            .scroll_to_item(self.preview.selected(), ScrollStrategy::Nearest);
        cx.notify();
    }

    /// Un clic en una fila la quita o la devuelve, como `x` sobre ella.
    fn click(&mut self, row: usize, cx: &mut Context<Self>) {
        self.preview.select(row);
        self.preview.toggle_selected();
        self.changed(cx);
    }

    fn render_header(&self, theme: &Theme) -> impl IntoElement {
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(theme.layout.picker_gap)
            .px(theme.layout.picker_padding_x)
            .py(theme.layout.picker_input_padding_y)
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .child(self.preview.title().to_owned()),
            )
            .child(
                div()
                    .ml_auto()
                    .flex_none()
                    .map(|element| theme.ui_small_text(element))
                    .text_color(theme.ui.picker_muted)
                    .child(self.preview.summary()),
            )
    }

    /// Los avisos del plan y, al final, por qué no se puede aplicar, uno
    /// por línea.
    fn render_notices(&self, theme: &Theme) -> Option<impl IntoElement> {
        let notices = self
            .preview
            .notices()
            .iter()
            .filter(|notice| !notice.blocks)
            .map(|notice| {
                div()
                    .text_color(theme.ui.picker_warning)
                    .child(notice.text.clone())
            });
        let blocked = self.preview.blocked().map(|blocked| {
            div()
                .debug_selector(|| "change-preview-blocked".to_owned())
                .text_color(theme.ui.error)
                .child(blocked_text(&blocked))
        });
        let lines: Vec<_> = notices.chain(blocked).collect();
        (!lines.is_empty()).then(|| {
            div()
                .flex_none()
                .flex()
                .flex_col()
                .px(theme.layout.picker_padding_x)
                .pb(theme.layout.picker_input_padding_y)
                .map(|element| theme.ui_small_text(element))
                .children(lines)
        })
    }

    fn render_list(&self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let row_height = theme.picker_row_height();
        let height = fit::list_height(
            self.preview.len(),
            theme.layout.picker_rows,
            f32::from(row_height),
            f32::from(theme.stroke.border),
        );
        let theme = theme.clone();
        let toggle_keys = keys(ChangePreviewAction::ToggleChange, cx);
        // En una ventana baja se encoge la lista (los demás son `flex_none`),
        // no el detalle ni los botones.
        div()
            .h(px(height))
            .min_h_0()
            .border_t(theme.stroke.border)
            .border_color(theme.ui.picker_border)
            .child(
                uniform_list(
                    "change-preview-rows",
                    self.preview.len(),
                    cx.processor(move |this, rows: Range<usize>, _window, cx| {
                        rows.map(|row| {
                            this.render_row(row, &theme, row_height, toggle_keys.clone(), cx)
                        })
                        .collect()
                    }),
                )
                .track_scroll(&self.scroll)
                .size_full(),
            )
    }

    fn render_row(
        &mut self,
        row: usize,
        theme: &Theme,
        row_height: Pixels,
        toggle_keys: Option<SharedString>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(change) = self.preview.row(row) else {
            return div().into_any_element();
        };
        let selected = row == self.preview.selected();
        let hover = theme.ui.picker_hover;
        // Un renombre ya dice su ruta en el cambio.
        let place = change.line.map(|line| format!("{}:{line}", change.path));
        let text = one_line(&change);
        let color = change_color(&change, theme);
        div()
            .id(row)
            .debug_selector(move || format!("change-preview-row-{row}"))
            .w_full()
            .h(row_height)
            .flex()
            .items_center()
            .gap(theme.layout.picker_gap)
            .px(theme.layout.picker_padding_x)
            .text_color(color)
            .when(selected, |row| row.bg(theme.ui.picker_selected))
            .when(!selected, |row| row.hover(move |style| style.bg(hover)))
            .on_click(cx.listener(move |this, _, _, cx| this.click(row, cx)))
            // El cambio se queda el ancho; la ruta se recorta antes.
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .when(change.dropped, Styled::line_through)
                    .child(text),
            )
            .child(
                div()
                    .flex_none()
                    .max_w(relative(theme.layout.picker_detail_share))
                    .truncate()
                    .text_ellipsis_start()
                    .map(|element| theme.ui_small_text(element))
                    .text_color(theme.ui.picker_muted)
                    .children(place),
            )
            .when_some(
                toggle_keys.filter(|_| selected && !change.fixed),
                |row, keys| {
                    row.child(
                        div()
                            .flex_none()
                            .map(|element| theme.ui_small_text(element))
                            .text_color(theme.ui.picker_muted)
                            .child(keys),
                    )
                },
            )
            .into_any_element()
    }

    /// El cambio elegido entero, en varias líneas: en la fila se recorta, y
    /// en un campo de la página el valor nuevo es lo que se confirma, así
    /// que va primero y lo de antes después. Su alto es fijo para que
    /// moverse entre filas no corra los botones.
    fn render_detail(&self, theme: &Theme) -> impl IntoElement {
        let detail = div()
            .debug_selector(|| "change-preview-detail".to_owned())
            .flex_none()
            .h(theme.change_preview_detail_height())
            .overflow_hidden()
            .px(theme.layout.picker_padding_x)
            .py(theme.layout.picker_input_padding_y)
            .border_t(theme.stroke.border)
            .border_color(theme.ui.picker_border);
        let Some(change) = self.preview.row(self.preview.selected()) else {
            return detail;
        };
        let after = if change.after.is_empty() {
            "→ se quita".to_owned()
        } else {
            format!("→ {}", change.after)
        };
        detail
            .text_color(change_color(&change, theme))
            .when(change.dropped, Styled::line_through)
            .child(
                div()
                    .debug_selector(|| "change-preview-detail-after".to_owned())
                    .child(after),
            )
            .when(!change.before.is_empty(), |detail| {
                detail.child(
                    div()
                        .text_color(theme.ui.picker_muted)
                        .child(format!("antes: {}", change.before)),
                )
            })
    }

    fn render_footer(&self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let blocked = self.preview.blocked().is_some();
        div()
            .flex_none()
            .flex()
            .justify_end()
            .gap(theme.layout.picker_gap)
            .px(theme.layout.picker_padding_x)
            .py(theme.layout.picker_input_padding_y)
            .border_t(theme.stroke.border)
            .border_color(theme.ui.picker_border)
            .child(button(
                "Cancelar",
                ChangePreviewAction::Cancel,
                false,
                theme,
                cx,
            ))
            .child(button(
                "Aplicar",
                ChangePreviewAction::Apply,
                blocked,
                theme,
                cx,
            ))
    }
}

impl Render for ChangePreviewView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        div()
            .key_context(key_context())
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_dispatch))
            .on_mouse_down_out(cx.listener(|_, _, _, cx| cx.emit(ChangePreviewEvent::Cancel)))
            .w(theme.layout.picker_width)
            .max_w(relative(0.9))
            .max_h(relative(1.))
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(theme.ui.picker_background)
            .border(theme.stroke.border)
            .border_color(theme.ui.picker_border)
            .rounded(theme.radius.lg)
            .child(self.render_header(&theme))
            .children(self.render_notices(&theme))
            .child(self.render_list(&theme, cx))
            .child(self.render_detail(&theme))
            .child(self.render_footer(&theme, cx))
    }
}

/// El cambio en una línea: los saltos se ven como `⏎`.
fn one_line(change: &PreviewRow<'_>) -> String {
    let flat = |text: &str| text.replace('\n', " ⏎ ");
    if change.before.is_empty() {
        format!("→ {}", flat(change.after))
    } else {
        format!("{} → {}", flat(change.before), flat(change.after))
    }
}

/// Apagado si se quitó; en rojo si pisaría cambios sin guardar.
fn change_color(change: &PreviewRow<'_>, theme: &Theme) -> Hsla {
    if change.dropped {
        theme.ui.picker_muted
    } else if change.unsaved.is_some() {
        theme.ui.error
    } else {
        theme.ui.foreground
    }
}

fn key_context() -> KeyContext {
    let mut key_context = KeyContext::new_with_defaults();
    key_context.add(context::CHANGE_PREVIEW);
    key_context
}

/// Un botón con su tecla al lado; `disabled` se pinta apagado y no hace
/// nada.
fn button(
    label: &'static str,
    action: ChangePreviewAction,
    disabled: bool,
    theme: &Theme,
    cx: &mut Context<ChangePreviewView>,
) -> AnyElement {
    let hover = theme.ui.picker_hover;
    let color = if disabled {
        theme.ui.picker_muted
    } else {
        theme.ui.foreground
    };
    div()
        .id(label)
        .debug_selector(move || format!("change-preview-{label}"))
        .flex()
        .items_center()
        .gap(theme.layout.picker_gap)
        .px(theme.layout.picker_padding_x)
        .rounded(theme.radius.md)
        .text_color(color)
        .when(!disabled, |button| {
            button.hover(move |style| style.bg(hover))
        })
        .on_click(cx.listener(move |_, _, window, cx| {
            window.dispatch_action(Box::new(Dispatch(Action::from(action).into())), cx);
        }))
        .child(label)
        .children(keys(action, cx).map(|keys| {
            div()
                .map(|element| theme.ui_small_text(element))
                .text_color(theme.ui.picker_muted)
                .child(keys)
        }))
        .into_any_element()
}

/// Las teclas de `action` en la vista previa.
fn keys(action: ChangePreviewAction, cx: &App) -> Option<SharedString> {
    AppKeymap::get(cx)
        .keys_for(action.into(), |view| view == View::ChangePreview)
        .map(SharedString::from)
}

fn blocked_text(blocked: &Blocked<'_>) -> String {
    match blocked {
        Blocked::Nothing => "no queda ningún cambio".to_owned(),
        Blocked::Notice(notice) => notice.text.clone(),
        Blocked::Unsaved(paths) => {
            let paths: Vec<&str> = paths.iter().map(|path| path.as_str()).collect();
            format!(
                "con cambios sin guardar: {}. Guárdalos o quita sus filas",
                paths.join(", ")
            )
        }
    }
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
