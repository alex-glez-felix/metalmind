//! Barra de estado de un panel: modo, si se graba una macro, archivo, `[+]`
//! si hay cambios sin guardar, el último mensaje o la línea de comandos, y
//! la posición del cursor. Modo, macro, mensaje y línea de comandos solo los
//! muestra el panel con el foco.

use gpui_kit::{
    Div, ParentElement as _, SharedString, Styled as _, div, prelude::FluentBuilder as _,
};
use nota_vim::{Mode, VisualKind};

use crate::theme::Theme;
use crate::workspace::Message;

/// Lo que muestra la barra, ya calculado por el editor.
#[derive(Clone, Debug)]
pub struct Status {
    /// `None` en un panel sin foco.
    pub mode: Option<Mode>,
    /// El registro de la macro que se está grabando.
    pub recording: Option<char>,
    pub file: SharedString,
    pub modified: bool,
    pub command_line: Option<CommandLine>,
    pub message: Option<Message>,
    /// Línea y columna en caracteres, desde 1.
    pub position: (usize, usize),
}

/// Lo escrito tras `:`, `/` o `?` y el preedit del IME.
#[derive(Clone, Debug)]
pub struct CommandLine {
    pub symbol: char,
    pub typed: String,
    pub preedit: String,
}

pub fn status_bar(status: Status, theme: &Theme) -> Div {
    let (line, column) = status.position;
    let center = match (status.command_line, status.message) {
        (Some(line), _) => command_line(line, theme),
        (None, Some(message)) => message_line(message, theme),
        (None, None) => div(),
    };
    let foreground = if status.mode.is_some() {
        theme.status_foreground
    } else {
        theme.status_unfocused_foreground
    };
    div()
        .flex()
        .flex_row()
        .gap(theme.status_gap)
        .px(theme.status_padding_x)
        .py(theme.status_padding_y)
        .bg(theme.status_background)
        .text_color(foreground)
        .border_t(theme.border_width)
        .border_color(theme.status_border)
        .children(status.mode.map(mode_label))
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
        Mode::Visual(VisualKind::Block) => "VISUAL BLOQUE",
        Mode::CommandLine => "COMANDO",
    }
}

impl CommandLine {
    /// Lo escrito tras su símbolo: `:w`, `/patrón` o `?patrón`.
    fn text(&self) -> String {
        format!("{}{}", self.symbol, self.typed)
    }
}

fn command_line(line: CommandLine, theme: &Theme) -> Div {
    let text = line.text();
    let preedit = line.preedit;
    div()
        .flex()
        .flex_row()
        .text_color(theme.foreground)
        .child(text)
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

#[cfg(test)]
mod tests {
    use nota_vim::Prompt;

    use super::*;

    #[test]
    fn la_linea_de_comandos_lleva_el_simbolo_de_su_prompt() {
        let cases = [
            (Prompt::Command, "w", ":w"),
            (Prompt::Search { backward: false }, "uno", "/uno"),
            (Prompt::Search { backward: true }, "dos", "?dos"),
        ];
        for (prompt, typed, expected) in cases {
            let line = CommandLine {
                symbol: prompt.symbol(),
                typed: typed.to_owned(),
                preedit: String::new(),
            };
            assert_eq!(line.text(), expected);
        }
    }
}
