//! La ventana de nota: el árbol de paneles como raíz, sin el `Root` de
//! gpui-kit, que se queda con `tab`, `shift-tab` y `ctrl-c` (04 §riesgo 1).

use std::time::Instant;

use gpui_kit::{App, AppContext as _, TitlebarOptions, WindowOptions};
use nota_config::Config;

use crate::actions;
use crate::document::Document;
use crate::theme::{NoMonoFont, Theme};
use crate::workspace::LineNumbers;
use crate::workspace_view::WorkspaceView;

#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error(transparent)]
    NoMonoFont(#[from] NoMonoFont),
    #[error("no se pudo abrir la ventana: {0:#}")]
    Window(#[source] anyhow::Error),
}

/// Abre `document` en una ventana nueva, con el tema y la fuente de `config`. La aplicación termina cuando se
/// cierra la última ventana.
///
/// # Errors
/// Si no hay fuente monoespaciada o la plataforma no puede abrir la ventana.
pub fn open(
    document: Document,
    config: &Config,
    launched: Instant,
    cx: &mut App,
) -> Result<(), OpenError> {
    cx.set_global(Theme::new(&config.theme, &config.settings.font, cx)?);
    actions::bind_keys(cx);
    cx.on_window_closed(|cx, _| {
        if cx.windows().is_empty() {
            cx.quit();
        }
    })
    .detach();
    let options = window_options(&document);
    let editor = config.settings.editor;
    let numbers = LineNumbers {
        absolute: editor.number,
        relative: editor.relativenumber,
    };
    cx.open_window(options, move |window, cx| {
        let view = cx.new(|cx| WorkspaceView::new(document, numbers, launched, cx));
        view.update(cx, |view, cx| view.focus(window, cx));
        let closing = view.downgrade();
        window.on_window_should_close(cx, move |_window, cx| {
            closing.update(cx, WorkspaceView::may_close).unwrap_or(true)
        });
        view
    })
    .map_err(OpenError::Window)?;
    Ok(())
}

fn window_options(document: &Document) -> WindowOptions {
    WindowOptions {
        titlebar: Some(TitlebarOptions {
            title: Some(format!("{} — nota", document.path().display()).into()),
            ..TitlebarOptions::default()
        }),
        app_id: Some("nota".to_owned()),
        ..WindowOptions::default()
    }
}
