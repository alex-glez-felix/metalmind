//! `metalmind [carpeta|archivo]`: abre el vault en una ventana, con el archivo
//! si se pasó uno; si el archivo no existe, empieza vacío y se crea al
//! guardar con `:w`. Qué vault se abre: [`target`]. Los subcomandos
//! (`metalmind add`, …) son la CLI del vault y, salvo `open`, nunca abren ventana.

mod cli;
mod commands;
mod gui;
mod target;

use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

use crate::cli::Command;
use crate::commands::CliError;
use crate::gui::{Session, StartError};

fn main() -> ExitCode {
    let launched = Instant::now();
    let cli = cli::Cli::parse_spanish();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let Some(command) = cli.command else {
        return match open(cli.path.as_deref(), cli.frame_log.as_deref(), launched) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("metalmind: {error}");
                ExitCode::FAILURE
            }
        };
    };
    let json = command.common().json;
    match run(command, launched) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            error.report(json);
            ExitCode::from(error.kind().exit_code())
        }
    }
}

fn open(
    path: Option<&Path>,
    frame_log: Option<&Path>,
    launched: Instant,
) -> Result<(), StartError> {
    let session = Session::load()?;
    let target = target::resolve(path, session.sources())?;
    let prepared = gui::prepare(session, &target)?;
    gui::run(prepared, frame_log, launched)
}

fn run(command: Command, launched: Instant) -> Result<(), CliError> {
    match command {
        Command::Add(args) => commands::add(args),
        Command::Search(args) => commands::search(&args),
        Command::Open(args) => {
            let note = commands::open(&args)?;
            let session = Session::load()?;
            let prepared = gui::prepare(session, &note.target())?;
            note.announce(args.common.json)?;
            Ok(gui::run(prepared, None, launched)?)
        }
    }
}
