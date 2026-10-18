//! La ventana: lo único del binario que toca GPUI. Preparar (config, vault y
//! documento) va aparte de abrir la ventana, para que `metalmind open --json`
//! anuncie la nota solo cuando ya no puede fallar al leerla.

use std::cell::RefCell;
use std::io;
use std::path::Path;
use std::rc::Rc;
use std::time::Instant;

use gpui_kit::App;
use metalmind_config::{Config, Diagnostic, Paths, Severity};
use metalmind_ui::{Document, FrameClock, OpenError, Startup};
use metalmind_vault::{FileError, Vault, VaultError};

use crate::target::{self, Sources, Target};

/// Lo que impide abrir la ventana.
#[derive(Debug, thiserror::Error)]
pub enum StartError {
    #[error("{0}")]
    Config(Diagnostic),
    #[error(transparent)]
    Target(#[from] target::TargetError),
    #[error(transparent)]
    Vault(#[from] VaultError),
    #[error(transparent)]
    File(#[from] FileError),
    #[error("no se pudo entrar en el vault: {0}")]
    EnterVault(io::Error),
    #[error("no se pudo crear el registro de frames: {0}")]
    FrameLog(io::Error),
    #[error(transparent)]
    Window(OpenError),
}

/// Las rutas de Metalmind y la config, lo primero que se lee.
pub struct Session {
    paths: Option<Paths>,
    config: Config,
}

impl Session {
    /// # Errors
    /// Solo si el tema embebido por defecto no carga.
    pub fn load() -> Result<Self, StartError> {
        let paths = Paths::detect()
            .inspect_err(|error| log::error!("{error}; se usa la configuración por defecto"))
            .ok();
        let config = load_config(paths.as_ref()).map_err(StartError::Config)?;
        Ok(Self { paths, config })
    }

    /// De dónde sale el vault si no se nombra uno.
    pub fn sources(&self) -> Sources {
        Sources::detect(self.paths.as_ref(), &self.config.settings)
    }
}

/// Lo que hace falta para abrir la ventana, ya leído.
pub struct Prepared {
    startup: Startup,
    config: Config,
}

/// Abre el vault de `target` y su archivo, si tiene; un archivo que no
/// existe empieza vacío.
///
/// # Errors
/// Si el vault no se puede abrir o el archivo no se puede leer.
pub fn prepare(session: Session, target: &Target) -> Result<Prepared, StartError> {
    let Session { paths, config } = session;
    let (vault, updates) = Vault::open(&target.vault)?;
    let file = target
        .file
        .as_deref()
        .map(|file| target::in_vault(vault.root(), file));
    std::env::set_current_dir(vault.root()).map_err(StartError::EnterVault)?;
    let document = match &file {
        Some(file) => Document::open(file)?,
        None => Document::unnamed(),
    };
    let startup = Startup {
        vault,
        updates,
        document,
        paths,
    };
    Ok(Prepared { startup, config })
}

/// Abre la ventana y vuelve cuando se cierra; con `frame_log`, escribe ahí
/// los tiempos de cada frame.
///
/// # Errors
/// Si no hay sesión gráfica, fuente monoespaciada o la plataforma no puede
/// abrir la ventana, o si no se puede crear `frame_log`.
pub fn run(
    prepared: Prepared,
    frame_log: Option<&Path>,
    launched: Instant,
) -> Result<(), StartError> {
    let Prepared { startup, config } = prepared;
    let clock = FrameClock::new(launched, frame_log).map_err(StartError::FrameLog)?;
    let failed = Rc::new(RefCell::new(None));
    let failed_in_app = Rc::clone(&failed);
    gpui_kit::application().run(move |cx| {
        if let Err(error) = metalmind_ui::open(startup, &config, clock, cx) {
            *failed_in_app.borrow_mut() = Some(error);
            quit_from_event_loop(cx);
        }
    });
    match failed.take() {
        Some(error) => Err(StartError::Window(error)),
        None => Ok(()),
    }
}

/// En Linux, el bucle de eventos olvida un `quit()` pedido antes de arrancar (#31); una tarea
/// del executor de primer plano solo corre ya dentro del bucle.
fn quit_from_event_loop(cx: &mut App) {
    cx.spawn(async |cx| cx.update(|cx| cx.quit())).detach();
}

/// La config del usuario; lo que esté mal se reporta y se usa el default.
/// Sin `paths` no hay de dónde leerla.
fn load_config(paths: Option<&Paths>) -> Result<Config, Diagnostic> {
    let Some(paths) = paths else {
        return Config::defaults();
    };
    let loaded = Config::load(paths)?;
    for diagnostic in &loaded.diagnostics {
        match diagnostic.severity {
            Severity::Warning => log::warn!("{diagnostic}"),
            Severity::Error => log::error!("{diagnostic}"),
        }
    }
    Ok(loaded.value)
}
