//! Argumentos de la línea de comandos.

use std::path::PathBuf;

use clap::{Arg, ArgAction, Args, CommandFactory as _, FromArgMatches as _, Parser, Subcommand};

/// Cuántos resultados da `metalmind search` si no se pide otra cosa.
const DEFAULT_LIMIT: usize = 50;

#[derive(Debug, Parser)]
#[command(
    version,
    about,
    disable_help_flag = true,
    disable_version_flag = true,
    disable_help_subcommand = true,
    args_conflicts_with_subcommands = true,
    subcommand_value_name = "COMANDO",
    subcommand_help_heading = "Comandos",
    arg(flag("help", 'h', ArgAction::Help, "Muestra esta ayuda")),
    arg(flag("version", 'V', ArgAction::Version, "Muestra la versión"))
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
    /// Carpeta a abrir como vault, o archivo markdown a abrir en el vault de
    /// su carpeta (si no existe, se crea al guardar con `:w`). Sin nada, la
    /// carpeta de `METALMIND_VAULT` o, si no está, el último vault abierto. Para
    /// un archivo que se llama como un comando: `metalmind -- add`.
    #[arg(value_name = "CARPETA|ARCHIVO")]
    pub path: Option<PathBuf>,
    /// Escribe los tiempos de cada frame en este archivo; es para el
    /// benchmark de la UI (docs/performance.md), por eso no sale en la ayuda.
    #[arg(long, value_name = "ARCHIVO", hide = true)]
    pub frame_log: Option<PathBuf>,
}

/// Los subcomandos no abren ventana, salvo `open`.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Crea una nota en el vault; sin tags, cae en el buzón
    Add(AddArgs),
    /// Busca rutas del vault con fuzzy, como `space f`
    Search(SearchArgs),
    /// Abre la app en una nota del vault
    Open(OpenArgs),
}

/// Lo que comparten todos los subcomandos.
#[derive(Debug, Args)]
pub struct Common {
    /// Carpeta del vault. Si falta: `METALMIND_VAULT`, el último abierto o `vault:`
    /// de config.yaml, como `metalmind` sin argumentos
    #[arg(long, value_name = "CARPETA")]
    pub vault: Option<PathBuf>,
    /// Salida en JSON, una línea; la forma está en docs/cli.md
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct AddArgs {
    /// Texto de la nota; varias palabras se unen con espacios. Si falta, se
    /// lee de stdin
    #[arg(value_name = "TEXTO")]
    pub text: Vec<String>,
    /// Tag de la nota, con o sin `#`; se puede repetir
    #[arg(short = 't', long = "tag", value_name = "TAG")]
    pub tags: Vec<String>,
    /// Título: va como `# Título` y da el nombre del archivo
    #[arg(long, value_name = "TÍTULO")]
    pub title: Option<String>,
    #[command(flatten)]
    pub common: Common,
}

#[derive(Debug, Args)]
pub struct SearchArgs {
    /// Palabras que tienen que coincidir todas, en orden dentro de la ruta
    #[arg(value_name = "CONSULTA")]
    pub query: String,
    /// Cuántos resultados, del mejor al peor; 50 si falta
    #[arg(short = 'n', long, value_name = "N", default_value_t = DEFAULT_LIMIT, hide_default_value = true)]
    pub limit: usize,
    #[command(flatten)]
    pub common: Common,
}

#[derive(Debug, Args)]
pub struct OpenArgs {
    /// Nota relativa a la raíz del vault, o absoluta dentro de él
    #[arg(value_name = "RUTA")]
    pub path: PathBuf,
    #[command(flatten)]
    pub common: Common,
}

impl Command {
    pub fn common(&self) -> &Common {
        match self {
            Self::Add(args) => &args.common,
            Self::Search(args) => &args.common,
            Self::Open(args) => &args.common,
        }
    }
}

impl Cli {
    /// Como [`Parser::parse`], pero con la ayuda en español también en cada
    /// subcomando.
    pub fn parse_spanish() -> Self {
        let mut matches = command().get_matches();
        Self::from_arg_matches_mut(&mut matches).unwrap_or_else(|error| error.exit())
    }
}

/// La definición completa: lo de clap viene en inglés, esto en el idioma de la app.
fn command() -> clap::Command {
    spanish(Cli::command()).mut_subcommands(|sub| {
        spanish(sub).disable_help_flag(true).arg(flag(
            "help",
            'h',
            ArgAction::Help,
            "Muestra esta ayuda",
        ))
    })
}

fn spanish(command: clap::Command) -> clap::Command {
    command
        .help_template("{about}\n\nuso: {usage}\n\n{all-args}")
        .mut_args(|arg| {
            let heading = if arg.is_positional() {
                "Argumentos"
            } else {
                "Opciones"
            };
            arg.help_heading(heading)
        })
}

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
    use clap::error::ErrorKind;

    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, ErrorKind> {
        let mut matches = command()
            .try_get_matches_from(std::iter::once("metalmind").chain(args.iter().copied()))
            .map_err(|error| error.kind())?;
        Cli::from_arg_matches_mut(&mut matches).map_err(|error| error.kind())
    }

    fn path(args: &[&str]) -> Result<PathBuf, ErrorKind> {
        parse(args).map(|cli| cli.path.unwrap_or_default())
    }

    #[test]
    fn argumentos() {
        let cases: [(&[&str], Result<PathBuf, ErrorKind>); 13] = [
            (&["a.md"], Ok("a.md".into())),
            (&["notas/b c.md"], Ok("notas/b c.md".into())),
            (&["--", "-raro.md"], Ok("-raro.md".into())),
            (&["--", "add"], Ok("add".into())),
            (&["add.md"], Ok("add.md".into())),
            (&["--help"], Err(ErrorKind::DisplayHelp)),
            (&["-h"], Err(ErrorKind::DisplayHelp)),
            (&["--version"], Err(ErrorKind::DisplayVersion)),
            (&["-V"], Err(ErrorKind::DisplayVersion)),
            (&[], Ok(PathBuf::new())),
            (&["a.md", "b.md"], Err(ErrorKind::ArgumentConflict)),
            (&["a.md", "add", "x"], Err(ErrorKind::ArgumentConflict)),
            (&["--raro"], Err(ErrorKind::UnknownArgument)),
        ];
        for (args, expected) in cases {
            assert_eq!(path(args), expected, "{args:?}");
        }
    }

    #[test]
    fn add_junta_las_palabras_y_repite_tags() {
        let cli = parse(&["add", "comprar", "leche", "-t", "casa", "--tag", "#super"]).unwrap();
        let Some(Command::Add(args)) = cli.command else {
            panic!("{cli:?}");
        };
        assert_eq!(args.text, ["comprar", "leche"]);
        assert_eq!(args.tags, ["casa", "#super"]);
        assert_eq!(args.title, None);
        assert!(!args.common.json);
    }

    #[test]
    fn opciones_comunes_en_cada_subcomando() {
        for args in [
            &["add", "--json", "--vault", "v", "x"][..],
            &["search", "q", "--vault", "v", "--json"],
            &["open", "--json", "a.md", "--vault=v"],
        ] {
            let cli = parse(args).unwrap();
            let common = cli.command.as_ref().unwrap().common();
            assert!(common.json, "{args:?}");
            assert_eq!(common.vault, Some("v".into()), "{args:?}");
        }
    }

    #[test]
    fn search_tiene_limite_por_defecto() {
        let Some(Command::Search(args)) = parse(&["search", "q"]).unwrap().command else {
            panic!();
        };
        assert_eq!(args.limit, DEFAULT_LIMIT);
        let help = command()
            .find_subcommand_mut("search")
            .unwrap()
            .render_help()
            .to_string();
        assert!(
            help.contains(&format!("{DEFAULT_LIMIT} si falta")),
            "{help}"
        );
        assert_eq!(
            parse(&["search"]).unwrap_err(),
            ErrorKind::MissingRequiredArgument
        );
        assert_eq!(
            parse(&["search", "q", "-n", "x"]).unwrap_err(),
            ErrorKind::ValueValidation
        );
    }

    #[test]
    fn un_archivo_no_lleva_opciones_de_subcomando() {
        assert_eq!(
            parse(&["a.md", "--json"]).unwrap_err(),
            ErrorKind::UnknownArgument
        );
    }

    #[test]
    fn el_registro_de_frames_va_con_la_ventana() {
        assert_eq!(parse(&["a.md"]).unwrap().frame_log, None);
        let cli = parse(&["--frame-log", "f.log", "a.md"]).unwrap();
        assert_eq!(cli.frame_log, Some("f.log".into()));
        assert_eq!(cli.path, Some("a.md".into()));
        assert_eq!(
            parse(&["--frame-log", "f.log"]).unwrap().path,
            None,
            "sin ruta abre el vault de siempre"
        );
        assert_eq!(
            parse(&["--frame-log", "f.log", "add", "x"]).unwrap_err(),
            ErrorKind::ArgumentConflict
        );
    }

    #[test]
    fn la_version_es_la_del_crate() {
        let version = command().render_version();
        assert_eq!(
            version,
            format!("metalmind {}\n", env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn la_ayuda_esta_en_espanol() {
        let help = command().render_help().to_string();
        for expected in [
            "uso: metalmind [CARPETA|ARCHIVO]",
            "metalmind <COMANDO>",
            "Comandos:",
            "add",
            "search",
            "open",
            "Argumentos:",
            "Opciones:",
            "-h, --help",
            "-V, --version",
        ] {
            assert!(help.contains(expected), "falta {expected:?} en:\n{help}");
        }
        assert!(!help.contains("frame-log"), "{help}");
    }

    #[test]
    fn la_ayuda_de_cada_subcomando_esta_en_espanol() {
        let mut command = command();
        for name in ["add", "search", "open"] {
            let help = command
                .find_subcommand_mut(name)
                .unwrap()
                .render_help()
                .to_string();
            for expected in [
                "uso: ",
                "Opciones:",
                "-h, --help",
                "Muestra esta ayuda",
                "--json",
            ] {
                assert!(
                    help.contains(expected),
                    "falta {expected:?} en {name}:\n{help}"
                );
            }
            assert!(!help.contains("Print help"), "{name}:\n{help}");
            assert!(!help.contains("Options:"), "{name}:\n{help}");
            assert!(!help.contains("[default"), "{name}:\n{help}");
        }
    }

    #[test]
    fn la_definicion_es_valida() {
        command().debug_assert();
    }
}
