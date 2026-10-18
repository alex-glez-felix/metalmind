//! La ventana de nota: la barra de botones, el panel lateral y el árbol de
//! paneles como raíz, sin el `Root` de gpui-kit, que se queda con `tab`,
//! `shift-tab` y `ctrl-c` (04 §riesgo 1).

use std::sync::mpsc;
use std::time::Instant;

use gpui_kit::{App, AppContext as _, TitlebarOptions, WindowOptions};
use nota_config::{Config, Paths};
use nota_vault::{Vault, VaultUpdate};

use crate::actions;
use crate::document::Document;
use crate::finder::{Prepared, SessionError, VaultSession};
use crate::picker;
use crate::session::{self, VaultState};
use crate::sidebar::Sidebar;
use crate::theme::{NoMonoFont, Theme};
use crate::workspace::{LineNumbers, Workspace};
use crate::workspace_view::WorkspaceView;

#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error(
        "no hay sesión gráfica: nota necesita Wayland o X11, y ni WAYLAND_DISPLAY ni DISPLAY \
         nombran una; ábrelo desde el escritorio"
    )]
    NoDisplay,
    #[error(transparent)]
    NoMonoFont(#[from] NoMonoFont),
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error(
        "no se pudo abrir la ventana: {0:#}\nnota dibuja con Vulkan u OpenGL: instala el driver \
         de la GPU (o mesa, que trae ambos por software) y compruébalo con `vulkaninfo --summary`"
    )]
    Window(#[source] anyhow::Error),
}

/// Lo que se abre: un vault con su watcher y el documento del primer panel.
pub struct Startup {
    pub vault: Vault,
    pub updates: mpsc::Receiver<VaultUpdate>,
    /// Un archivo del vault (o de fuera), o un buffer sin nombre.
    pub document: Document,
    /// Dónde va el estado de la UI; sin directorio del usuario no se guarda.
    pub paths: Option<Paths>,
}

/// Abre `startup` en una ventana nueva, con el tema y la fuente de
/// `config`, y lo anota como el último vault. La aplicación termina cuando
/// se cierra la última ventana.
///
/// # Errors
/// Si no hay sesión gráfica ni fuente monoespaciada, no arranca el hilo
/// del watcher o la plataforma no puede abrir la ventana.
pub fn open(
    startup: Startup,
    config: &Config,
    launched: Instant,
    cx: &mut App,
) -> Result<(), OpenError> {
    // Sin display, GPUI arranca headless y la ventana se abriría sin que nadie la vea.
    if cx.graphical_environment().is_none() {
        return Err(OpenError::NoDisplay);
    }
    cx.set_global(Theme::new(config, cx)?);
    actions::bind_keys(cx);
    picker::bind_keys(cx);
    cx.on_window_closed(|cx, _| {
        if cx.windows().is_empty() {
            cx.quit();
        }
    })
    .detach();
    let Startup {
        vault,
        updates,
        document,
        paths,
    } = startup;
    let state = remember(paths.as_ref(), &vault);
    let prepared = Prepared::new(vault, updates, paths.as_ref())?;
    let options = window_options(&document);
    let editor = config.settings.editor;
    let numbers = LineNumbers {
        absolute: editor.number,
        relative: editor.relativenumber,
    };
    let scrolloff = usize::try_from(editor.scrolloff).unwrap_or(usize::MAX);
    let tabstop = usize::from(editor.tabstop.get());
    let file = document.path().map(ToOwned::to_owned);
    cx.open_window(options, move |window, cx| {
        let workspace = cx.new(|_| Workspace::new(document, numbers, scrolloff, tabstop));
        let session = cx.new(|cx| VaultSession::new(prepared, cx));
        let sidebar = cx.new(|cx| Sidebar::new(&session, &state.panel, workspace.clone(), cx));
        if let Some(file) = &file {
            sidebar.update(cx, |sidebar, cx| sidebar.reveal(file, cx));
        }
        let view = cx.new(|cx| {
            WorkspaceView::new(
                workspace,
                sidebar.clone(),
                session,
                paths,
                launched,
                window,
                cx,
            )
        });
        view.update(cx, |view, cx| {
            if file.is_none() && sidebar.read(cx).is_visible() {
                view.focus_sidebar(window, cx);
            } else {
                view.focus(window, cx);
            }
        });
        let closing = view.downgrade();
        window.on_window_should_close(cx, move |_window, cx| {
            closing.update(cx, WorkspaceView::may_close).unwrap_or(true)
        });
        view
    })
    .map_err(OpenError::Window)?;
    Ok(())
}

/// El estado guardado de `vault`, que además queda como el último abierto.
fn remember(paths: Option<&Paths>, vault: &Vault) -> VaultState {
    let Some(paths) = paths else {
        return VaultState::default();
    };
    if let Err(error) = session::remember_vault(paths, vault.root()) {
        log::warn!("no se anotó el último vault: {error}");
    }
    VaultState::load(paths, vault.root())
}

fn window_options(document: &Document) -> WindowOptions {
    WindowOptions {
        titlebar: Some(TitlebarOptions {
            title: Some(format!("{} — nota", document.display_name()).into()),
            ..TitlebarOptions::default()
        }),
        app_id: Some("nota".to_owned()),
        ..WindowOptions::default()
    }
}
