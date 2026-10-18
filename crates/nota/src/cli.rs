//! Argumentos de la línea de comandos.

use std::path::PathBuf;

use clap::{Arg, ArgAction, Parser};

#[derive(Debug, Parser)]
#[command(
    version,
    about,
    help_template = "{about}\n\nuso: {usage}\n\n{all-args}",
    disable_help_flag = true,
    disable_version_flag = true,
    arg(flag("help", 'h', ArgAction::Help, "Muestra esta ayuda")),
    arg(flag("version", 'V', ArgAction::Version, "Muestra la versión"))
)]
pub struct Cli {
    /// Archivo markdown a abrir; si no existe, se crea al guardar con `:w`.
    #[arg(value_name = "ARCHIVO", help_heading = "Argumentos")]
    pub path: PathBuf,
}

/// Las de clap traen el texto en inglés; estas, en el idioma de la app.
fn flag(name: &'static str, short: char, action: ArgAction, help: &'static str) -> Arg {
    Arg::new(name)
        .short(short)
        .long(name)
        .action(action)
        .help(help)
        .help_heading("Opciones")
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory as _;
    use clap::error::ErrorKind;

    use super::*;

    fn parse(args: &[&str]) -> Result<PathBuf, ErrorKind> {
        Cli::try_parse_from(std::iter::once("nota").chain(args.iter().copied()))
            .map(|cli| cli.path)
            .map_err(|error| error.kind())
    }

    #[test]
    fn argumentos() {
        let cases: [(&[&str], Result<PathBuf, ErrorKind>); 10] = [
            (&["a.md"], Ok("a.md".into())),
            (&["notas/b c.md"], Ok("notas/b c.md".into())),
            (&["--", "-raro.md"], Ok("-raro.md".into())),
            (&["--help"], Err(ErrorKind::DisplayHelp)),
            (&["-h"], Err(ErrorKind::DisplayHelp)),
            (&["--version"], Err(ErrorKind::DisplayVersion)),
            (&["-V"], Err(ErrorKind::DisplayVersion)),
            (&[], Err(ErrorKind::MissingRequiredArgument)),
            (&["a.md", "b.md"], Err(ErrorKind::UnknownArgument)),
            (&["--raro"], Err(ErrorKind::UnknownArgument)),
        ];
        for (args, expected) in cases {
            assert_eq!(parse(args), expected, "{args:?}");
        }
    }

    #[test]
    fn la_version_es_la_del_crate() {
        let version = Cli::command().render_version();
        assert_eq!(version, format!("nota {}\n", env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn la_ayuda_esta_en_espanol() {
        let help = Cli::command().render_help().to_string();
        for expected in [
            "uso: nota <ARCHIVO>",
            "Argumentos:",
            "Opciones:",
            "-h, --help",
            "-V, --version",
        ] {
            assert!(help.contains(expected), "falta {expected:?} en:\n{help}");
        }
    }

    #[test]
    fn la_definicion_es_valida() {
        Cli::command().debug_assert();
    }
}
