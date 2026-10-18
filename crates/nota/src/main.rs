//! `nota [carpeta|archivo]`: abre el vault en una ventana, con el archivo
//! si se pasó uno; si el archivo no existe, empieza vacío y se crea al
//! guardar con `:w`. Qué vault se abre: [`target`].

mod cli;
mod target;

use std::cell::Cell;
use std::process::ExitCode;
use std::rc::Rc;
use std::time::Instant;

use clap::Parser as _;
use gpui_kit::App;
use nota_config::{Config, Diagnostic, Paths, Severity};
use nota_ui::{Document, Startup};
use nota_vault::{FileError, Vault, VaultError};

fn main() -> ExitCode {
    let launched = Instant::now();
    let cli = cli::Cli::parse();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    match prepare(&cli) {
        Ok((startup, config)) => run(startup, config, launched),
        Err(error) => {
            eprintln!("nota: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Lo que impide abrir la ventana.
#[derive(Debug, thiserror::Error)]
enum StartError {
    #[error("{0}")]
    Config(Diagnostic),
    #[error(transparent)]
    Target(#[from] target::TargetError),
    #[error(transparent)]
    Vault(#[from] VaultError),
    #[error(transparent)]
    File(#[from] FileError),
}

fn prepare(cli: &cli::Cli) -> Result<(Startup, Config), StartError> {
    let paths = Paths::detect()
        .inspect_err(|error| log::error!("{error}; se usa la configuración por defecto"))
        .ok();
    let config = load_config(paths.as_ref()).map_err(StartError::Config)?;
    let remembered = paths.as_ref().and_then(nota_ui::last_vault);
    let configured = dirs::home_dir().and_then(|home| config.settings.vault(&home));
    let target = target::resolve(cli.path.as_deref(), remembered, configured)?;
    let (vault, updates) = Vault::open(&target.vault)?;
    let document = match &target.file {
        Some(file) => Document::open(file)?,
        None => Document::unnamed(),
    };
    let startup = Startup {
        vault,
        updates,
        document,
        paths,
    };
    Ok((startup, config))
}

fn run(startup: Startup, config: Config, launched: Instant) -> ExitCode {
    let failed = Rc::new(Cell::new(false));
    let failed_in_app = Rc::clone(&failed);
    gpui_kit::application().run(move |cx| {
        if let Err(error) = nota_ui::open(startup, &config, launched, cx) {
            eprintln!("nota: {error}");
            failed_in_app.set(true);
            quit_from_event_loop(cx);
        }
    });
    if failed.get() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
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
