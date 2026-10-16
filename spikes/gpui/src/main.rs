//! Spike de GPUI para `nota`. Ver `README.md` para correrlo y la checklist.

mod cli;
mod document;
mod editor;
mod events;
mod fixture;
mod hud;
mod keys;
mod kit;
mod metrics;
mod report;
mod scroll;
mod session;
mod shell;
mod tokens;
mod utf16;

use std::fs::{self, File};
use std::io::{self, BufWriter};
use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

use cli::{Command, View};
use session::{Session, SessionInfo};

/// Muestra en la terminal qué GPU eligió wgpu y los eventos de teclado.
const DEFAULT_LOG: &str = "warn,gpui_wgpu=info,nota_spike_gpui=info";

fn main() -> ExitCode {
    let launched = Instant::now();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(DEFAULT_LOG)).init();
    match cli::parse(std::env::args().skip(1)) {
        Ok(Command::Generate { out, megabytes }) => generate(&out, megabytes),
        Ok(Command::Open { view, path, bench }) => open(view, &path, bench, launched),
        Err(error) => {
            eprintln!("{error}\n\n{}", cli::USAGE);
            ExitCode::from(2)
        }
    }
}

fn generate(out: &Path, megabytes: u64) -> ExitCode {
    match write_fixture(out, megabytes) {
        Ok(stats) => {
            println!(
                "{}: {} bytes, {} líneas",
                out.display(),
                stats.bytes,
                stats.lines
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("no se pudo generar {}: {error}", out.display());
            ExitCode::FAILURE
        }
    }
}

/// Escribe a un temporal y renombra, para no dejar un archivo a medias.
fn write_fixture(out: &Path, megabytes: u64) -> io::Result<fixture::Stats> {
    if let Some(dir) = out.parent() {
        fs::create_dir_all(dir)?;
    }
    let partial = out.with_extension("parcial");
    let stats = fixture::generate(
        &mut BufWriter::new(File::create(&partial)?),
        megabytes * 1024 * 1024,
    )?;
    fs::rename(&partial, out)?;
    Ok(stats)
}

fn open(view: View, path: &Path, bench: bool, launched: Instant) -> ExitCode {
    let reading = Instant::now();
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("no se pudo leer {}: {error}", path.display());
            return ExitCode::FAILURE;
        }
    };
    let read = reading.elapsed();
    let info = SessionInfo {
        view,
        path: path.to_owned(),
        bytes: text.len(),
        lines: 0,
        bench,
    };
    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);
        editor::bind_keys(cx);
        shell::bind_keys(cx);
        cx.set_global(Session::new(info, launched));
        cx.on_app_quit(|cx| {
            println!("{}", report::render(cx.global::<Session>()));
            async {}
        })
        .detach();
        cx.on_window_closed(|cx, _| cx.quit()).detach();
        if let Err(error) = shell::open(view, &text, read, bench, cx) {
            log::error!("no se pudo abrir la ventana: {error:#}");
            cx.quit();
        }
    });
    ExitCode::SUCCESS
}
