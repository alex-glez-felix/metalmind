//! Barra de estado de un panel: modo, si se graba una macro, archivo, `[+]`
//! si hay cambios sin guardar (o, con autoguardado, si está guardado), el
//! último mensaje o la línea de comandos, y la posición del cursor. Modo,
//! macro, mensaje y línea de comandos solo los muestra el panel con el foco.

use gpui_kit::{
    Div, ParentElement as _, SharedString, Styled as _, div, prelude::FluentBuilder as _,
};
use metalmind_vim::{Mode, VisualKind};

use crate::theme::Theme;
use crate::workspace::{Message, SaveState};

/// Lo que muestra la barra, ya calculado por el editor.
#[derive(Clone, Debug)]
pub struct Status {
    /// `None` en un panel sin foco.
    pub mode: Option<Mode>,
    /// El registro de la macro que se está grabando.
    pub recording: Option<char>,
    pub file: SharedString,
    pub modified: bool,
    /// El indicador del autoguardado, en lugar del `[+]`; `None` si está
    /// apagado.
    pub save: Option<SaveState>,
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
        // Con algo que decir, un mínimo legible: el nombre cede antes que el
        // mensaje quede en un `…` suelto.
        (Some(line), _) => command_line(line, theme).min_w(theme.status.message_min_width),
        (None, Some(message)) => message_line(message, theme).min_w(theme.status.message_min_width),
        (None, None) => div().min_w_0(),
    };
    let foreground = if status.mode.is_some() {
        theme.status.foreground
    } else {
        theme.status.unfocused_foreground
    };
    div()
        .flex()
        .flex_row()
        // En un panel más angosto que lo mínimo, se recorta el final sin salirse.
        .overflow_hidden()
        .gap(theme.status.gap)
        .px(theme.status.padding_x)
        .py(theme.status.padding_y)
        .bg(theme.status.background)
        .text_color(foreground)
        .border_t(theme.border_width)
        .border_color(theme.status.border)
        .children(status.mode.map(|mode| fixed().child(mode_label(mode))))
        .when_some(status.recording, |bar, register| {
            bar.child(fixed().child(format!("grabando @{register}")))
        })
        .child(file_label(status.file, status.save, status.modified, theme))
        .child(center.flex_grow_1().truncate())
        .child(fixed().child(format!("{line}:{column}")))
}

/// Lo que nunca se recorta: modo, macro, indicador y posición.
fn fixed() -> Div {
    div().flex_shrink_0().whitespace_nowrap()
}

/// El nombre se recorta por el medio, para que se vean el principio y la
/// extensión; el indicador o el `[+]` siempre se ven enteros. La barra es de
/// una sola línea: cuando no alcanza, el nombre cede frente al mensaje lo que
/// diga el tema (`layout.status_file_shrink`).
fn file_label(file: SharedString, save: Option<SaveState>, modified: bool, theme: &Theme) -> Div {
    let mark = match save {
        Some(state) => Some(save_label(state, theme)),
        None => modified.then(|| fixed().text_color(theme.modified).child(" [+]")),
    };
    div()
        .flex()
        .flex_row()
        .min_w_0()
        .flex_shrink(theme.status.file_shrink)
        .child(
            div()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis_middle()
                .child(file),
        )
        .children(mark)
}

/// Sin animación ni un "guardando…" entre medio: solo cambia de texto.
fn save_label(state: SaveState, theme: &Theme) -> Div {
    let (text, color) = match state {
        SaveState::Saved => (" · guardado", theme.status.saved),
        SaveState::Unsaved => (" · sin guardar", theme.status.unsaved),
        SaveState::ChangedOnDisk => (
            " · cambió en disco: :w! o :e!",
            theme.status.changed_on_disk,
        ),
    };
    fixed().text_color(color).child(text)
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
    use metalmind_vim::Prompt;

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
