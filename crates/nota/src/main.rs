//! `nota archivo.md`: abre el archivo en una ventana; si no existe, empieza
//! vacío y se crea al guardar con `:w`.

mod cli;

use std::cell::Cell;
use std::path::Path;
use std::process::ExitCode;
use std::rc::Rc;
use std::time::Instant;

use clap::Parser as _;
use gpui_kit::App;
use nota_config::{Config, Diagnostic, Paths, Severity};
use nota_ui::Document;

fn main() -> ExitCode {
    let launched = Instant::now();
    let cli = cli::Cli::parse();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    open(&cli.path, launched)
}

fn open(path: &Path, launched: Instant) -> ExitCode {
    let config = match load_config() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("nota: {error}");
            return ExitCode::FAILURE;
        }
    };
    let document = match Document::open(path) {
        Ok(document) => document,
        Err(error) => {
            eprintln!("nota: {error}");
            return ExitCode::FAILURE;
        }
    };
    let failed = Rc::new(Cell::new(false));
    let failed_in_app = Rc::clone(&failed);
    gpui_kit::application().run(move |cx| {
        if let Err(error) = nota_ui::open(document, &config, launched, cx) {
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
fn load_config() -> Result<Config, Diagnostic> {
    let loaded = match Paths::detect() {
        Ok(paths) => Config::load(&paths)?,
        Err(error) => {
            log::error!("{error}; se usa la configuración por defecto");
            return Config::defaults();
        }
    };
    for diagnostic in &loaded.diagnostics {
        match diagnostic.severity {
            Severity::Warning => log::warn!("{diagnostic}"),
            Severity::Error => log::error!("{diagnostic}"),
        }
    }
    Ok(loaded.value)
}
