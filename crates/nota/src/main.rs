//! `nota archivo.md`: abre el archivo en una ventana; si no existe, empieza
//! vacío y se crea al guardar con `:w`.

use std::cell::Cell;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;
use std::rc::Rc;
use std::time::Instant;

use nota_ui::Document;

const USAGE: &str = "uso: nota <archivo.md>";

#[derive(Debug, PartialEq, Eq)]
enum Cli {
    Open(PathBuf),
    Help,
}

#[derive(Debug, PartialEq, Eq)]
struct UsageError;

fn parse(mut args: impl Iterator<Item = OsString>) -> Result<Cli, UsageError> {
    let (Some(arg), None) = (args.next(), args.next()) else {
        return Err(UsageError);
    };
    match arg.to_str() {
        Some("-h" | "--help") => Ok(Cli::Help),
        Some(flag) if flag.starts_with('-') => Err(UsageError),
        _ => Ok(Cli::Open(arg.into())),
    }
}

fn main() -> ExitCode {
    let launched = Instant::now();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    match parse(std::env::args_os().skip(1)) {
        Ok(Cli::Open(path)) => open(&path, launched),
        Ok(Cli::Help) => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Err(UsageError) => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn open(path: &std::path::Path, launched: Instant) -> ExitCode {
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
        if let Err(error) = nota_ui::open(document, launched, cx) {
            eprintln!("nota: {error}");
            failed_in_app.set(true);
            cx.quit();
        }
    });
    if failed.get() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> impl Iterator<Item = OsString> {
        list.iter()
            .map(OsString::from)
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn argumentos() {
        let cases: [(&[&str], Result<Cli, UsageError>); 7] = [
            (&["a.md"], Ok(Cli::Open("a.md".into()))),
            (&["notas/b c.md"], Ok(Cli::Open("notas/b c.md".into()))),
            (&["--help"], Ok(Cli::Help)),
            (&["-h"], Ok(Cli::Help)),
            (&[], Err(UsageError)),
            (&["a.md", "b.md"], Err(UsageError)),
            (&["--version"], Err(UsageError)),
        ];
        for (list, expected) in cases {
            assert_eq!(parse(args(list)), expected, "{list:?}");
        }
    }
}
