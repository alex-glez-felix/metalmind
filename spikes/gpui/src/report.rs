//! Reporte final en markdown, listo para pegar en `docs/research/04-spike-gpui.md`.

use std::env;
use std::time::Duration;

use crate::metrics::{Summary, fps, millis};
use crate::session::{Session, SessionInfo};

/// Variables que dicen en qué sesión gráfica se corrió.
const ENVIRONMENT: &[&str] = &[
    "XDG_SESSION_TYPE",
    "XDG_CURRENT_DESKTOP",
    "WAYLAND_DISPLAY",
    "DISPLAY",
];

pub fn render(session: &Session) -> String {
    let metrics = &session.metrics;
    let mut rows = vec![
        ("vista", session.info.view.name().to_owned()),
        ("archivo", file(&session.info)),
        ("corrida", run_kind(&session.info).to_owned()),
        ("carga del archivo", optional(metrics.load())),
        ("arranque → primer frame", optional(metrics.first_frame())),
        ("frames dibujados", metrics.frames().to_string()),
        (
            "intervalo entre frames p50 / p99 / máx",
            summary(metrics.intervals()),
        ),
        (
            "fps sostenidos (1000 / p50)",
            metrics
                .intervals()
                .map_or_else(|| "—".to_owned(), |s| fps(s.p50)),
        ),
        ("CPU por frame p50 / p99 / máx", summary(metrics.cpu())),
    ];
    for name in ENVIRONMENT {
        rows.push((name, env::var(name).unwrap_or_else(|_| "—".to_owned())));
    }
    table(&rows)
}

fn file(info: &SessionInfo) -> String {
    format!(
        "{} ({:.1} MB, {} líneas)",
        info.path.display(),
        megabytes(info.bytes),
        info.lines
    )
}

fn run_kind(info: &SessionInfo) -> &'static str {
    if info.bench {
        "benchmark de scroll automático"
    } else {
        "manual"
    }
}

fn megabytes(bytes: usize) -> f64 {
    f64::from(u32::try_from(bytes / 1024).unwrap_or(u32::MAX)) / 1024.0
}

fn optional(duration: Option<Duration>) -> String {
    duration.map_or_else(|| "—".to_owned(), millis)
}

fn summary(summary: Option<Summary>) -> String {
    summary.map_or_else(
        || "sin muestras".to_owned(),
        |s| {
            format!(
                "{} / {} / {} ({} muestras)",
                millis(s.p50),
                millis(s.p99),
                millis(s.max),
                s.count
            )
        },
    )
}

fn table(rows: &[(&str, String)]) -> String {
    let body: Vec<String> = rows
        .iter()
        .map(|(name, value)| format!("| {name} | {value} |"))
        .collect();
    format!("\n| métrica | valor |\n|---|---|\n{}\n", body.join("\n"))
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;
    use crate::cli::View;

    #[test]
    fn reporte_con_y_sin_muestras() {
        let launched = Instant::now();
        let info = SessionInfo {
            view: View::Vim,
            path: "fixtures/grande.md".into(),
            bytes: 50 * 1024 * 1024,
            lines: 600_000,
            bench: true,
        };
        let mut session = Session::new(info, launched);
        let empty = render(&session);
        assert!(empty.contains("| CPU por frame p50 / p99 / máx | sin muestras |"));

        session.metrics.record_load(Duration::from_millis(120));
        let frame = launched + Duration::from_millis(300);
        session.metrics.frame_started(frame);
        session
            .metrics
            .frame_started(frame + Duration::from_millis(7));
        session
            .metrics
            .frame_painted(frame, frame + Duration::from_millis(2));
        let full = render(&session);
        for row in [
            "| archivo | fixtures/grande.md (50.0 MB, 600000 líneas) |",
            "| corrida | benchmark de scroll automático |",
            "| carga del archivo | 120.00 ms |",
            "| arranque → primer frame | 302.00 ms |",
            "| fps sostenidos (1000 / p50) | 143 |",
        ] {
            assert!(full.contains(row), "falta {row:?} en:\n{full}");
        }
    }
}
