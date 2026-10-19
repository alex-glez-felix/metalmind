//! Barra de estado de un panel: modo, si se graba una macro, archivo, `[+]`
//! si hay cambios sin guardar (o, con autoguardado, si está guardado), el
//! último mensaje o la línea de comandos, y la posición del cursor. Modo,
//! macro, mensaje y línea de comandos solo los muestra el panel con el foco.

use gpui_kit::{
    Div, ParentElement as _, SharedString, Styled as _, div, prelude::FluentBuilder as _, relative,
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
        (Some(line), _) => Some(command_line(line, theme)),
        (None, Some(message)) => Some(message_line(message, theme)),
        (None, None) => None,
    };
    let foreground = if status.mode.is_some() {
        theme.status.foreground
    } else {
        theme.status.unfocused_foreground
    };
    let mark = match status.save {
        Some(state) => Some(save_label(state, theme)),
        None => status
            .modified
            .then(|| fixed().text_color(theme.modified).child(" [+]")),
    };
    div()
        .flex()
        .flex_row()
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
        .child(middle(status.file, mark, center, theme))
        .child(fixed().child(format!("{line}:{column}")))
}

/// Lo que nunca se recorta: modo, macro, indicador y posición.
fn fixed() -> Div {
    div().flex_shrink_0().whitespace_nowrap()
}

/// Entre el modo y la posición: el nombre de la nota, el indicador (o el
/// `[+]`) y el mensaje. El mensaje no cede: mide lo que dice, hasta su parte
/// de este espacio, y se lee entero; el nombre toma lo que queda y se
/// recorta por el medio, para que se vean el principio y la extensión. La
/// barra es de una sola línea; en un panel más angosto que lo mínimo, se
/// recorta el final sin salirse.
fn middle(file: SharedString, mark: Option<Div>, center: Option<Div>, theme: &Theme) -> Div {
    div()
        .flex()
        .flex_row()
        .flex_grow_1()
        .min_w_0()
        .overflow_hidden()
        .child(
            div()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis_middle()
                .child(file),
        )
        .children(mark)
        .children(center.map(|center| {
            center
                .flex_shrink_0()
                .max_w(relative(theme.status.message_max_width))
                .ml(theme.status.gap)
                .truncate()
        }))
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
