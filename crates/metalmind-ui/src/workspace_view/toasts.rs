//! Los avisos del [`Workspace`] en toasts, arriba a la derecha: el ícono de
//! su nivel y el texto entero. Se cierran solos según su nivel
//! ([`Message::lifetime`]), con `esc` en Normal o con un clic.

use std::collections::HashMap;

use gpui_kit::{
    AnyElement, Context, Div, Hsla, InteractiveElement as _, IntoElement as _, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Task, div, prelude::FluentBuilder as _,
};

use super::WorkspaceView;
use crate::theme::Theme;
#[cfg(doc)]
use crate::workspace::Workspace;
use crate::workspace::{Message, MessageId};

/// El temporizador de cada toast que se cierra solo; soltarlo lo cancela.
pub(super) type ToastTimers = HashMap<MessageId, Task<()>>;

impl WorkspaceView {
    /// Un temporizador por toast que se ve y se cierra solo, ni uno más.
    pub(super) fn schedule_toasts(&mut self, cx: &mut Context<Self>) {
        let toasts: Vec<_> = self
            .workspace
            .read(cx)
            .messages()
            .toasts()
            .into_iter()
            .map(|(id, message)| (id, message.lifetime()))
            .collect();
        self.toast_timers
            .retain(|id, _| toasts.iter().any(|(shown, _)| shown == id));
        for (id, lifetime) in toasts {
            let Some(lifetime) = lifetime else {
                continue;
            };
            self.toast_timers.entry(id).or_insert_with(|| {
                cx.spawn(async move |view, cx| {
                    cx.background_executor().timer(lifetime).await;
                    // Sin la vista, no queda toast que cerrar.
                    let _ = view.update(cx, |view, cx| view.dismiss_toast(id, cx));
                })
            });
        }
    }

    fn dismiss_toast(&mut self, id: MessageId, cx: &mut Context<Self>) {
        self.workspace.update(cx, |workspace, cx| {
            workspace.dismiss_message(id);
            cx.notify();
        });
    }

    pub(super) fn render_toasts(
        &self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let workspace = self.workspace.read(cx);
        let toasts = workspace.messages().toasts();
        if toasts.is_empty() {
            return None;
        }
        let rows = toasts
            .into_iter()
            .enumerate()
            .map(|(index, (id, message))| {
                toast(message, theme)
                    .id(("toast", index))
                    .on_click(cx.listener(move |view, _, _, cx| view.dismiss_toast(id, cx)))
            });
        Some(
            div()
                .absolute()
                .top(theme.layout.toast_margin)
                .right(theme.layout.toast_margin)
                .w(theme.layout.toast_width)
                .flex()
                .flex_col()
                .gap(theme.layout.toast_gap)
                .children(rows)
                .into_any_element(),
        )
    }
}

/// Tapa lo de abajo para que el clic que lo cierra no le llegue.
fn toast(message: &Message, theme: &Theme) -> Div {
    let (icon, color) = icon(message, theme);
    div()
        .occlude()
        .flex()
        .flex_row()
        .items_start()
        .gap(theme.layout.toast_gap)
        .px(theme.layout.toast_padding_x)
        .py(theme.layout.toast_padding_y)
        .bg(theme.ui.toast_background)
        .border(theme.stroke.border)
        .border_color(theme.ui.toast_border)
        .rounded(theme.radius.lg)
        .text_color(theme.ui.foreground)
        .child(div().flex_none().text_color(color).child(icon))
        .child(div().flex_1().min_w_0().child(message.text().to_owned()))
        .child(
            div()
                .flex_none()
                .map(|hint| theme.ui_small_text(hint))
                .text_color(theme.ui.toast_hint)
                .child("esc"),
        )
}

/// Una forma distinta por nivel, para no depender solo del color.
fn icon(message: &Message, theme: &Theme) -> (&'static str, Hsla) {
    match message {
        Message::Info(_) => ("●", theme.ui.toast_info),
        Message::Warning(_) => ("▲", theme.ui.toast_warning),
        Message::Error(_) => ("◆", theme.ui.toast_error),
    }
}
