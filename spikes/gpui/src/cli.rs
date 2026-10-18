use std::path::PathBuf;

pub const USAGE: &str = "\
uso:
  metalmind-spike-gpui gen [--mb N] [--out RUTA]   genera el markdown de prueba (50 MB por defecto)
  metalmind-spike-gpui vim RUTA [--bench]          vista propia: metalmind-buffer + metalmind-vim
  metalmind-spike-gpui kit RUTA [--bench]          editor de gpui-component, para comparar

--bench hace scroll automático durante unos segundos, imprime el reporte y sale.";

const DEFAULT_MEGABYTES: u64 = 50;
const DEFAULT_FIXTURE: &str = "fixtures/grande.md";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Vim,
    Kit,
}

impl View {
    pub fn name(self) -> &'static str {
        match self {
            Self::Vim => "vim",
            Self::Kit => "kit",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Generate {
        out: PathBuf,
        megabytes: u64,
    },
    Open {
        view: View,
        path: PathBuf,
        bench: bool,
    },
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum CliError {
    #[error("falta el subcomando")]
    MissingCommand,
    #[error("subcomando desconocido: {0}")]
    UnknownCommand(String),
    #[error("falta la ruta del archivo")]
    MissingPath,
    #[error("{0} necesita un valor")]
    MissingValue(&'static str),
    #[error("no es un número válido de MB: {0}")]
    InvalidMegabytes(String),
    #[error("argumento desconocido: {0}")]
    UnknownArgument(String),
}

pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Command, CliError> {
    let mut args = args.into_iter();
    match args.next().as_deref() {
        Some("gen") => parse_generate(args),
        Some("vim") => parse_open(View::Vim, args),
        Some("kit") => parse_open(View::Kit, args),
        Some(other) => Err(CliError::UnknownCommand(other.to_owned())),
        None => Err(CliError::MissingCommand),
    }
}

fn parse_generate(mut args: impl Iterator<Item = String>) -> Result<Command, CliError> {
    let mut out = PathBuf::from(DEFAULT_FIXTURE);
    let mut megabytes = DEFAULT_MEGABYTES;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => out = args.next().ok_or(CliError::MissingValue("--out"))?.into(),
            "--mb" => {
                let value = args.next().ok_or(CliError::MissingValue("--mb"))?;
                megabytes = value
                    .parse()
                    .ok()
                    .filter(|mb| *mb > 0)
                    .ok_or(CliError::InvalidMegabytes(value))?;
            }
            _ => return Err(CliError::UnknownArgument(arg)),
        }
    }
    Ok(Command::Generate { out, megabytes })
}

fn parse_open(view: View, args: impl Iterator<Item = String>) -> Result<Command, CliError> {
    let mut path = None;
    let mut bench = false;
    for arg in args {
        match arg.as_str() {
            "--bench" => bench = true,
            flag if flag.starts_with("--") => return Err(CliError::UnknownArgument(arg)),
            _ if path.is_none() => path = Some(PathBuf::from(arg)),
            _ => return Err(CliError::UnknownArgument(arg)),
        }
    }
    let path = path.ok_or(CliError::MissingPath)?;
    Ok(Command::Open { view, path, bench })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(line: &str) -> Result<Command, CliError> {
        parse(line.split_whitespace().map(str::to_owned))
    }

    #[test]
    fn gen_usa_valores_por_defecto() {
        assert_eq!(
            run("gen"),
            Ok(Command::Generate {
                out: DEFAULT_FIXTURE.into(),
                megabytes: DEFAULT_MEGABYTES,
            })
        );
    }

    #[test]
    fn gen_acepta_tamano_y_salida() {
        assert_eq!(
            run("gen --mb 5 --out /tmp/x.md"),
            Ok(Command::Generate {
                out: "/tmp/x.md".into(),
                megabytes: 5,
            })
        );
    }

    #[test]
    fn gen_rechaza_tamano_invalido() {
        assert_eq!(
            run("gen --mb 0"),
            Err(CliError::InvalidMegabytes("0".into()))
        );
        assert_eq!(
            run("gen --mb mucho"),
            Err(CliError::InvalidMegabytes("mucho".into()))
        );
        assert_eq!(run("gen --mb"), Err(CliError::MissingValue("--mb")));
    }

    #[test]
    fn abrir_en_cada_vista() {
        assert_eq!(
            run("vim a.md"),
            Ok(Command::Open {
                view: View::Vim,
                path: "a.md".into(),
                bench: false,
            })
        );
        assert_eq!(
            run("kit --bench a.md"),
            Ok(Command::Open {
                view: View::Kit,
                path: "a.md".into(),
                bench: true,
            })
        );
    }

    #[test]
    fn abrir_exige_una_sola_ruta() {
        assert_eq!(run("vim"), Err(CliError::MissingPath));
        assert_eq!(
            run("vim a.md b.md"),
            Err(CliError::UnknownArgument("b.md".into()))
        );
        assert_eq!(
            run("vim --rapido a.md"),
            Err(CliError::UnknownArgument("--rapido".into()))
        );
    }

    #[test]
    fn subcomando_desconocido_o_ausente() {
        assert_eq!(run(""), Err(CliError::MissingCommand));
        assert_eq!(run("ver"), Err(CliError::UnknownCommand("ver".into())));
    }
}
