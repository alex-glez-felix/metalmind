//! Barra de estado: modo, si se graba una macro, archivo, `[+]` si hay cambios sin guardar, el
//! último mensaje o la línea de comandos, y la posición del cursor.

use gpui_kit::{
    Div, ParentElement as _, SharedString, Styled as _, div, prelude::FluentBuilder as _,
};
use nota_vim::{Mode, VisualKind};

use crate::theme::Theme;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    Info(String),
    Error(String),
}

/// Lo que muestra la barra, ya calculado por el editor.
#[derive(Clone, Debug)]
pub struct Status {
    pub mode: Mode,
    /// El registro de la macro que se está grabando.
    pub recording: Option<char>,
    pub file: SharedString,
    pub modified: bool,
    /// Lo escrito tras `:` y el preedit del IME, si se está escribiendo un comando.
    pub command_line: Option<(String, String)>,
    pub message: Option<Message>,
    /// Línea y columna en caracteres, desde 1.
    pub position: (usize, usize),
}

pub fn status_bar(status: Status, theme: &Theme) -> Div {
    let (line, column) = status.position;
    let center = match (status.command_line, status.message) {
        (Some((typed, preedit)), _) => command_line(&typed, preedit, theme),
        (None, Some(message)) => message_line(message, theme),
        (None, None) => div(),
    };
    div()
        .flex()
        .flex_row()
        .gap(theme.status_gap)
        .px(theme.status_padding_x)
        .py(theme.status_padding_y)
        .bg(theme.status_background)
        .text_color(theme.status_foreground)
        .border_t(theme.border_width)
        .border_color(theme.status_border)
        .child(mode_label(status.mode))
        .when_some(status.recording, |bar, register| {
            bar.child(format!("grabando @{register}"))
        })
        .child(
            div()
                .flex()
                .flex_row()
                .child(status.file)
                .when(status.modified, |file| {
                    file.child(div().text_color(theme.modified).child(" [+]"))
                }),
        )
        .child(center.flex_1().min_w_0().overflow_hidden())
        .child(format!("{line}:{column}"))
}

fn mode_label(mode: Mode) -> &'static str {
    match mode {
        Mode::Normal => "NORMAL",
        Mode::Insert => "INSERTAR",
        Mode::OperatorPending(_) => "OPERADOR",
        Mode::Visual(VisualKind::Char) => "VISUAL",
        Mode::Visual(VisualKind::Line) => "VISUAL LÍNEA",
        Mode::CommandLine => "COMANDO",
    }
}

fn command_line(typed: &str, preedit: String, theme: &Theme) -> Div {
    div()
        .flex()
        .flex_row()
        .text_color(theme.foreground)
        .child(format!(":{typed}"))
        .when(!preedit.is_empty(), |line| {
            line.child(div().underline().child(preedit))
        })
        .child(div().w(theme.caret_width).h_full().bg(theme.caret))
}

fn message_line(message: Message, theme: &Theme) -> Div {
    match message {
        Message::Info(text) => div().child(text),
        Message::Error(text) => div().text_color(theme.error).child(text),
    }
}
