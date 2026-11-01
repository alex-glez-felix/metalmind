//! Los avisos del [`Workspace`] en toasts, arriba a la derecha: el ícono de
//! su nivel, el texto entero y, si trae uno, un botón con su tecla, como
//! "Deshacer". Se cierran solos según su nivel ([`Message::lifetime`]), con
//! `esc` en Normal o con un clic.

use std::collections::HashMap;
use std::time::Duration;

use gpui_kit::{
    AnyElement, App, Context, Div, Hsla, InteractiveElement as _, IntoElement as _,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, Task, div,
    prelude::FluentBuilder as _,
};
use metalmind_keymap::{Action, View};

use super::WorkspaceView;
use crate::actions::{AppKeymap, Dispatch};
use crate::theme::Theme;
#[cfg(doc)]
use crate::workspace::Workspace;
use crate::workspace::{Message, ToastAction, ToastId};

/// El temporizador de cada toast que se cierra solo; soltarlo lo cancela.
pub(super) type ToastTimers = HashMap<ToastId, Task<()>>;

/// Lo menos que se ve un toast con botón: da tiempo de llevar el ratón.
const WITH_ACTION: Duration = Duration::from_secs(10);

impl WorkspaceView {
    /// Un temporizador por toast que se ve y se cierra solo, ni uno más. Lo
    /// llama cada frame.
    pub(super) fn schedule_toasts(&mut self, cx: &mut Context<Self>) {
        let messages = self.workspace.read(cx).messages();
        let toasts: Vec<_> = messages
            .toasts()
            .into_iter()
            .map(|(toast, message)| {
                let lifetime = message.lifetime();
                let lifetime = match messages.action(toast) {
                    Some(_) => lifetime.map(|lifetime| lifetime.max(WITH_ACTION)),
                    None => lifetime,
                };
                (toast, lifetime)
            })
            .collect();
        self.toast_timers
            .retain(|toast, _| toasts.iter().any(|(shown, _)| shown == toast));
        for (toast, lifetime) in toasts {
            let Some(lifetime) = lifetime else {
                continue;
            };
            self.toast_timers.entry(toast).or_insert_with(|| {
                cx.spawn(async move |view, cx| {
                    cx.background_executor().timer(lifetime).await;
                    // Sin la vista, no queda toast que cerrar.
                    let _ = view.update(cx, |view, cx| view.dismiss_toast(toast, cx));
                })
            });
        }
    }

    fn dismiss_toast(&mut self, toast: ToastId, cx: &mut Context<Self>) {
        self.workspace.update(cx, |workspace, cx| {
            workspace.dismiss_toast(toast);
            cx.notify();
        });
    }

    pub(super) fn render_toasts(
        &self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let messages = self.workspace.read(cx).messages();
        let toasts = messages.toasts();
        if toasts.is_empty() {
            return None;
        }
        let rows: Vec<_> = toasts
            .into_iter()
            .enumerate()
            .map(|(index, (shown, message))| {
                let button = messages
                    .action(shown)
                    .map(|action| action_button(index, action, theme, cx));
                toast(message, button, theme)
                    .id(("toast", index))
                    .on_click(cx.listener(move |view, _, _, cx| view.dismiss_toast(shown, cx)))
            })
            .collect();
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
fn toast(message: &Message, button: Option<AnyElement>, theme: &Theme) -> Div {
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
        .children(button)
        .child(
            div()
                .flex_none()
                .map(|hint| theme.ui_small_text(hint))
                .text_color(theme.ui.toast_hint)
                .child("esc"),
        )
}

/// El botón de `action` con la tecla de su comando en el editor. Su clic
/// no cierra el toast como el resto: lo hace el comando, si ya no vale.
fn action_button(index: usize, action: ToastAction, theme: &Theme, cx: &App) -> AnyElement {
    let command = action.command;
    let keys = AppKeymap::get(cx).keys_for(command.into(), |view| view == View::Editor);
    let hover = theme.ui.toast_button_hover;
    div()
        .id(("toast-action", index))
        .debug_selector(move || format!("toast-action-{index}"))
        .flex_none()
        .flex()
        .items_center()
        .gap(theme.layout.toast_gap)
        .px(theme.layout.toast_padding_x)
        .rounded(theme.radius.md)
        .text_color(theme.ui.foreground)
        .hover(move |style| style.bg(hover))
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            window.dispatch_action(Box::new(Dispatch(Action::from(command).into())), cx);
        })
        .child(action.label)
        .children(keys.map(|keys| {
            div()
                .map(|hint| theme.ui_small_text(hint))
                .text_color(theme.ui.toast_hint)
                .child(keys)
        }))
        .into_any_element()
}

/// Una forma distinta por nivel, para no depender solo del color.
fn icon(message: &Message, theme: &Theme) -> (&'static str, Hsla) {
    match message {
        Message::Info(_) => ("●", theme.ui.toast_info),
        Message::Warning(_) => ("▲", theme.ui.toast_warning),
        Message::Error(_) => ("◆", theme.ui.toast_error),
    }
}
