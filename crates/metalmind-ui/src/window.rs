//! La ventana de Metalmind: la barra de botones, el panel lateral y el árbol de
//! paneles como raíz, sin el `Root` de gpui-kit, que se queda con `tab`,
//! `shift-tab` y `ctrl-c` (04 §riesgo 1).

use std::rc::Rc;
use std::sync::mpsc;

use gpui_kit::{AnyWindowHandle, App, AppContext as _, TitlebarOptions, WindowOptions};
use metalmind_config::{Config, Paths};
use metalmind_keymap::Keymap;
use metalmind_vault::{Vault, VaultUpdate};
use metalmind_vim::Markdown;

use crate::actions::{self, AppKeymap};
use crate::disk::VaultDisk;
use crate::document::Document;
use crate::finder::{Prepared, SessionError, VaultSession};
use crate::frames::FrameClock;
use crate::new_note::NoteSettings;
use crate::session::{self, VaultState};
use crate::sidebar::Sidebar;
use crate::theme::{NoMonoFont, Theme};
use crate::workspace::{Autosave, LineNumbers, Options, SystemClock, Workspace};
use crate::workspace_view::WorkspaceView;

#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error(
        "no hay sesión gráfica: Metalmind necesita Wayland o X11, y ni WAYLAND_DISPLAY ni DISPLAY \
         nombran una; ábrelo desde el escritorio"
    )]
    NoDisplay,
    #[error(transparent)]
    NoMonoFont(#[from] NoMonoFont),
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error(
        "no se pudo abrir la ventana: {0:#}\nMetalmind dibuja con Vulkan u OpenGL: instala el driver \
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
/// `config`, y lo anota como el último vault; los tiempos de cada frame van
/// a `clock`. La aplicación termina cuando se cierra la última ventana.
///
/// # Errors
/// Si no hay sesión gráfica ni fuente monoespaciada, no arranca el hilo
/// del watcher o la plataforma no puede abrir la ventana.
pub fn open(
    startup: Startup,
    config: &Config,
    clock: FrameClock,
    cx: &mut App,
) -> Result<(), OpenError> {
    // Sin display, GPUI arranca headless y la ventana se abriría sin que nadie la vea.
    if cx.graphical_environment().is_none() {
        return Err(OpenError::NoDisplay);
    }
    cx.set_global(clock);
    cx.on_window_closed(|cx, _| {
        if cx.windows().is_empty() {
            cx.quit();
        }
    })
    .detach();
    open_window(startup, config, cx).map(drop)
}

/// La ventana de [`open`], en la plataforma que tenga `cx`: también la de
/// pruebas de GPUI, sin display, para contar lo que cuesta pintarla.
///
/// # Errors
/// Si no hay fuente monoespaciada, no arranca el hilo del watcher o la
/// plataforma no puede abrir la ventana.
pub fn open_window(
    startup: Startup,
    config: &Config,
    cx: &mut App,
) -> Result<AnyWindowHandle, OpenError> {
    cx.set_global(Theme::new(config, cx)?);
    actions::bind_keys(Keymap::default(), cx);
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
    let workspace_options = Options {
        scrolloff: usize::try_from(editor.scrolloff).unwrap_or(usize::MAX),
        tabstop: usize::from(editor.tabstop.get()),
        expandtab: editor.expandtab,
        gutter_marks: editor.gutter_marks,
        markdown: Markdown {
            continue_lists: config.settings.markdown.continue_lists,
            indent_list_children: config.settings.markdown.indent_list_children,
        },
    };
    let autosave = editor.autosave.then_some(editor.autosave_delay_ms.get());
    let file = document.path().map(ToOwned::to_owned);
    let notes = NoteSettings {
        templates_folder: config.settings.templates_folder.clone(),
        daily: config.settings.daily.clone(),
    };
    cx.open_window(options, move |window, cx| {
        let session = cx.new(|cx| VaultSession::new(prepared, cx));
        let disk = Rc::new(VaultDisk::new(session.read(cx).vault()));
        let workspace = cx.new(|cx| {
            let keymap = AppKeymap::get(cx);
            let mut workspace = Workspace::new(document, numbers, workspace_options, keymap);
            workspace.set_disk(disk);
            workspace
                .set_autosave(autosave.map(|delay| Autosave::new(delay, Rc::new(SystemClock))));
            workspace
        });
        let sidebar =
            cx.new(|cx| Sidebar::new(&session, &state.panel, workspace.clone(), window, cx));
        if let Some(file) = &file {
            sidebar.update(cx, |sidebar, cx| sidebar.reveal(file, cx));
        }
        let view = cx.new(|cx| {
            WorkspaceView::new(
                workspace,
                sidebar.clone(),
                session,
                paths,
                notes,
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
        window.on_window_should_close(cx, move |window, cx| {
            closing
                .update(cx, |view, cx| view.may_close(window, cx))
                .unwrap_or(true)
        });
        view
    })
    .map(Into::into)
    .map_err(OpenError::Window)
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
            title: Some(format!("{} — Metalmind", document.display_name()).into()),
            ..TitlebarOptions::default()
        }),
        app_id: Some("metalmind".to_owned()),
        ..WindowOptions::default()
    }
}
