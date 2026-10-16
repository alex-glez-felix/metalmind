//! Overlay de medición: modo, posición, tiempos de frame y últimos eventos.

use std::time::Duration;

use gpui_kit::{IntoElement, ParentElement as _, Styled as _, div};

use crate::metrics::{Summary, fps, millis};
use crate::session::Session;
use crate::tokens::Tokens;

/// Lo que cada vista cuenta de sí al overlay.
#[derive(Clone, Debug, Default)]
pub struct Status {
    pub mode: &'static str,
    pub line: usize,
    pub column: usize,
    pub lines: usize,
    pub preedit: Option<String>,
    pub events: Vec<String>,
}

pub fn hud(status: Status, session: &Session, tokens: &Tokens) -> impl IntoElement {
    let metrics = &session.metrics;
    let position = format!(
        "{}  línea {}/{}  col {}  ·  carga {}  ·  primer frame {}",
        status.mode,
        status.line,
        status.lines,
        status.column,
        optional(metrics.load()),
        optional(metrics.first_frame()),
    );
    let frames = format!(
        "frames {}  ·  intervalo {}  ·  CPU {}  último {}",
        metrics.frames(),
        live_interval(metrics.live_intervals()),
        live(metrics.live_cpu()),
        optional(metrics.last_cpu()),
    );
    let preedit = status.preedit.map(|text| format!("preedit: {text:?}"));
    div()
        .flex()
        .flex_col()
        .flex_none()
        .w_full()
        .px_2()
        .py_1()
        .border_t_1()
        .border_color(tokens.border)
        .bg(tokens.muted)
        .text_color(tokens.muted_foreground)
        .font_family(tokens.mono_font_family.clone())
        .text_size(tokens.mono_font_size)
        .child(div().text_color(tokens.foreground).child(position))
        .child(div().text_color(tokens.foreground).child(frames))
        .children(preedit.map(|text| div().child(text)))
        .children(status.events.into_iter().map(|event| {
            let color = if event.starts_with("ERROR") {
                tokens.danger
            } else {
                tokens.muted_foreground
            };
            div().text_color(color).child(event)
        }))
}

fn optional(duration: Option<Duration>) -> String {
    duration.map_or_else(|| "—".to_owned(), millis)
}

fn live(summary: Option<Summary>) -> String {
    summary.map_or_else(
        || "—".to_owned(),
        |s| format!("p50 {} p99 {}", millis(s.p50), millis(s.p99)),
    )
}

fn live_interval(summary: Option<Summary>) -> String {
    summary.map_or_else(
        || "—".to_owned(),
        |s| format!("{} (~{} fps)", live(Some(s)), fps(s.p50)),
    )
}
