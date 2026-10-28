//! Barra de estado de un panel: modo, si se graba una macro, archivo, `[+]`
//! si hay cambios sin guardar (o, con autoguardado, si está guardado), la
//! línea de comandos, el focus y la posición del cursor. Modo, macro y
//! línea de comandos solo los muestra el panel con el foco; los avisos van
//! en toasts.
//! Sobre la línea de comandos va la lista del autocompletado de su argumento.

use gpui_kit::{
    AnyElement, Div, ParentElement as _, SharedString, Styled as _, div,
    prelude::FluentBuilder as _, relative,
};
use metalmind_vim::{Mode, VisualKind};

use crate::focus_indicator::focus_indicator;
use crate::theme::Theme;
use crate::workspace::SaveState;

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
    /// Línea y columna en caracteres, desde 1.
    pub position: (usize, usize),
    /// `focus: #rust`, si hay focus.
    pub focus: Option<SharedString>,
}

/// Lo escrito tras `:`, `/` o `?` y el preedit del IME.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandLine {
    pub symbol: char,
    pub typed: String,
    pub preedit: String,
}

/// Lo que va entre el modo y la posición.
#[derive(Debug, PartialEq)]
enum Middle {
    /// La nota y su indicador.
    Note,
    /// Lo que se teclea tras `:`, `/` o `?`: ocupa todo el medio, como la
    /// línea de comandos de vim.
    Typing(CommandLine),
}

impl Middle {
    fn of(command_line: Option<CommandLine>) -> Self {
        command_line.map_or(Self::Note, Self::Typing)
    }
}

/// `menu` es la lista del autocompletado de lo que se teclea tras `:`.
pub fn status_bar(status: Status, menu: Option<AnyElement>, theme: &Theme) -> Div {
    let (line, column) = status.position;
    let foreground = if status.mode.is_some() {
        theme.ui.status_foreground
    } else {
        theme.ui.status_unfocused_foreground
    };
    let mark = match status.save {
        Some(state) => Some(save_label(state, theme)),
        None => status
            .modified
            .then(|| fixed().text_color(theme.ui.modified).child(" [+]")),
    };
    div()
        .flex()
        .flex_row()
        .gap(theme.layout.status_gap)
        .px(theme.layout.status_padding_x)
        .py(theme.layout.status_padding_y)
        .bg(theme.ui.status_background)
        .text_color(foreground)
        .border_t(theme.stroke.border)
        .border_color(theme.ui.status_border)
        .children(status.mode.map(|mode| fixed().child(mode_label(mode))))
        .when_some(status.recording, |bar, register| {
            bar.child(fixed().child(format!("grabando @{register}")))
        })
        .child(match Middle::of(status.command_line) {
            Middle::Typing(line) => command_line(line, menu, theme),
            Middle::Note => note(status.file, mark),
        })
        .children(status.focus.map(|label| {
            focus_indicator("status-focus", label, theme)
                .flex_none()
                .max_w(relative(theme.layout.status_focus_max_width))
        }))
        .child(fixed().child(format!("{line}:{column}")))
}

/// Lo que nunca se recorta: modo, macro, indicador y posición.
fn fixed() -> Div {
    div().flex_shrink_0().whitespace_nowrap()
}

/// Entre el modo y la posición: el nombre de la nota y el indicador (o el
/// `[+]`). El nombre se recorta por el medio, para que se vean el principio
/// y la extensión. La barra es de una sola línea; en un panel más angosto
/// que lo mínimo, se recorta el final sin salirse.
fn note(file: SharedString, mark: Option<Div>) -> Div {
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
}

/// Sin animación ni un "guardando…" entre medio: solo cambia de texto.
fn save_label(state: SaveState, theme: &Theme) -> Div {
    let (text, color) = match state {
        SaveState::Saved => (" · guardado", theme.ui.status_saved),
        SaveState::Unsaved => (" · sin guardar", theme.ui.status_unsaved),
        SaveState::ChangedOnDisk => (
            " · cambió en disco: :w! o :e!",
            theme.ui.status_changed_on_disk,
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

/// Si no cabe, se recorta el principio de lo escrito: el preedit y el cursor,
/// que es donde se escribe, siempre se ven. `menu` se pinta desde su
/// principio.
fn command_line(line: CommandLine, menu: Option<AnyElement>, theme: &Theme) -> Div {
    let text = line.text();
    let preedit = line.preedit;
    div()
        .flex()
        .flex_row()
        .flex_grow_1()
        .min_w_0()
        .text_color(theme.ui.foreground)
        .children(menu)
        .child(
            div()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis_start()
                .child(text),
        )
        .when(!preedit.is_empty(), |line| {
            line.child(fixed().underline().child(preedit))
        })
        .child(
            fixed()
                .w(theme.stroke.caret)
                .h_full()
                .bg(theme.cursor.insert),
        )
}

#[cfg(test)]
mod tests {
    use metalmind_vim::Prompt;

    use super::*;

    #[test]
    fn lo_que_se_teclea_ocupa_todo_el_medio() {
        let line = CommandLine {
            symbol: ':',
            typed: "una línea de comandos larga".to_owned(),
            preedit: String::new(),
        };
        assert_eq!(Middle::of(Some(line.clone())), Middle::Typing(line));
        assert_eq!(Middle::of(None), Middle::Note);
    }

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
