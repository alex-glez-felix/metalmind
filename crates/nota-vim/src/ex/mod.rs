//! Lo escrito tras `:`: un rango y un comando, los que corre el motor
//! (`:s`, `:d`, `:g`, `:normal`, …) y los que corre quien aloja el editor
//! ([`Invocation`]).

mod address;
mod args;
mod global;
mod join;
mod sort;

pub(crate) use address::{Address, Base, LineRange};
pub(crate) use args::RegisterAndCount;
pub(crate) use global::GlobalCommand;
pub use global::GlobalMiss;
pub(crate) use join::join;
pub(crate) use sort::{SortOptions, sort};

use self::address::skip_blanks;
use crate::indent::Shift;
use crate::substitute::SubstituteCommand;
use crate::{CommandError, Invocation};

/// Lo escrito tras `:`: un comando que corre quien aloja el editor o uno
/// del motor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ParsedLine {
    Host(Invocation),
    Engine(ExLine),
}

/// Un comando del motor con el rango que se le escribió.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExLine {
    pub range: Option<LineRange>,
    pub command: ExCommand,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExCommand {
    /// Solo un rango, como `:12`: va a su última línea.
    GoTo,
    Substitute(SubstituteCommand),
    /// `:noh`: deja de resaltar la búsqueda hasta la próxima.
    ClearSearchHighlight,
    Delete(RegisterAndCount),
    Yank(RegisterAndCount),
    /// `:m {dirección}`: debajo de esa línea; `0` es arriba de todo.
    Move(Address),
    /// `:t` o `:co`.
    Copy(Address),
    Join {
        /// `:j` las separa con un espacio; `:j!` las pega tal cual.
        spaces: bool,
        count: Option<usize>,
    },
    /// `:>` o `:<`, repetidos para varios niveles (`:>>`).
    Shift {
        shift: Shift,
        levels: usize,
        count: Option<usize>,
    },
    /// `:normal {teclas}`; sin mapeos en nota, `:normal!` es lo mismo.
    Normal(String),
    Global(GlobalCommand),
    Sort(SortOptions),
}

/// Los comandos que corre el motor, con cuántas letras bastan para
/// abreviarlos, como en vim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Name {
    Substitute,
    ClearSearchHighlight,
    Delete,
    Yank,
    Move,
    Copy,
    Join,
    Normal,
    Global,
    VGlobal,
    Sort,
}

const NAMES: &[(&str, usize, Name)] = &[
    ("substitute", 1, Name::Substitute),
    ("nohlsearch", 3, Name::ClearSearchHighlight),
    ("delete", 1, Name::Delete),
    ("yank", 1, Name::Yank),
    ("move", 1, Name::Move),
    ("t", 1, Name::Copy),
    ("copy", 2, Name::Copy),
    ("join", 1, Name::Join),
    ("normal", 4, Name::Normal),
    ("global", 1, Name::Global),
    ("vglobal", 1, Name::VGlobal),
    ("sort", 3, Name::Sort),
];

impl ParsedLine {
    /// Como vim, ignora espacios y `:` de más; una línea vacía no es nada.
    ///
    /// # Errors
    /// Si no es un comando conocido o su rango o sus argumentos no valen.
    pub fn parse(line: &str) -> Result<Option<Self>, CommandError> {
        let typed = line.trim_start_matches([' ', ':']);
        if typed.trim_end().is_empty() {
            return Ok(None);
        }
        let (range, rest) = LineRange::parse(typed)?;
        let rest = skip_blanks(rest);
        let command = match parse_command(rest)? {
            Some(command) => command,
            None if rest.trim_end().is_empty() => ExCommand::GoTo,
            None if range.is_none() => {
                return Invocation::parse(line).map(|host| host.map(Self::Host));
            }
            None => return Err(ranged_host_error(typed, rest)),
        };
        if range.is_some() && command == ExCommand::ClearSearchHighlight {
            return Err(CommandError::NoRangeAllowed);
        }
        Ok(Some(Self::Engine(ExLine { range, command })))
    }
}

/// `None` si no es un comando del motor.
fn parse_command(text: &str) -> Result<Option<ExCommand>, CommandError> {
    if let Some(shift) = text.chars().next().filter(|&c| c == '>' || c == '<') {
        let name_len = text.len() - text.trim_start_matches(shift).len();
        let levels = name_len;
        let shift = if shift == '>' {
            Shift::Right
        } else {
            Shift::Left
        };
        let (bang, args) = split_bang(&text[name_len..]);
        no_bang(bang)?;
        let count = args::count(args)?;
        return Ok(Some(ExCommand::Shift {
            shift,
            levels,
            count,
        }));
    }
    let name_len = text
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(text.len());
    let (typed, rest) = text.split_at(name_len);
    let Some(name) = NAMES
        .iter()
        .find(|&&(full, min, _)| typed.len() >= min && full.starts_with(typed))
        .map(|&(_, _, name)| name)
    else {
        return Ok(None);
    };
    let (bang, args) = split_bang(rest);
    let command = match name {
        // `!` puede ser su delimitador: `:s!a!b!`.
        Name::Substitute => ExCommand::Substitute(SubstituteCommand::parse(rest)?),
        Name::ClearSearchHighlight => {
            no_bang(bang)?;
            args::no_trailing(args)?;
            ExCommand::ClearSearchHighlight
        }
        Name::Delete => ExCommand::Delete(register_and_count(bang, args)?),
        Name::Yank => ExCommand::Yank(register_and_count(bang, args)?),
        Name::Move => ExCommand::Move(destination(bang, args)?),
        Name::Copy => ExCommand::Copy(destination(bang, args)?),
        Name::Join => ExCommand::Join {
            spaces: !bang,
            count: args::count(args)?,
        },
        Name::Normal => {
            let keys = skip_blanks(args);
            if keys.is_empty() {
                return Err(CommandError::ArgumentRequired);
            }
            ExCommand::Normal(keys.to_owned())
        }
        Name::Global => ExCommand::Global(GlobalCommand::parse(args, bang)?),
        Name::VGlobal => ExCommand::Global(GlobalCommand::parse(args, true)?),
        Name::Sort => ExCommand::Sort(SortOptions::parse(bang, args)?),
    };
    Ok(Some(command))
}

/// Un comando de quien aloja el editor con rango, como `:'<,'>w` tras `:` en
/// Visual. Si no existe, el error lleva lo escrito con el rango, como vim.
fn ranged_host_error(typed: &str, rest: &str) -> CommandError {
    match Invocation::parse(rest) {
        Ok(Some(host)) => host.command.range_error(rest.trim_end()),
        Ok(None) | Err(CommandError::Unknown(_)) => {
            CommandError::Unknown(typed.trim_end().to_owned())
        }
        Err(error) => error,
    }
}

fn split_bang(text: &str) -> (bool, &str) {
    text.strip_prefix('!')
        .map_or((false, text), |rest| (true, rest))
}

fn no_bang(bang: bool) -> Result<(), CommandError> {
    if bang {
        Err(CommandError::NoBangAllowed)
    } else {
        Ok(())
    }
}

fn register_and_count(bang: bool, args: &str) -> Result<RegisterAndCount, CommandError> {
    no_bang(bang)?;
    RegisterAndCount::parse(args)
}

/// La línea debajo de la que van `:m` y `:t`. Como `ex_copymove` de vim,
/// lo que sigue a la dirección no se mira.
fn destination(bang: bool, args: &str) -> Result<Address, CommandError> {
    no_bang(bang)?;
    let (address, _) = Address::parse(args)?;
    address.ok_or(CommandError::InvalidRange)
}

#[cfg(test)]
mod tests {
    use super::address::Term;
    use super::*;
    use crate::Command;

    fn parse(line: &str) -> Result<Option<ParsedLine>, CommandError> {
        ParsedLine::parse(line)
    }

    fn command(line: &str) -> Result<ExCommand, CommandError> {
        match parse(line)? {
            Some(ParsedLine::Engine(parsed)) => Ok(parsed.command),
            other => panic!("{line:?} no es del motor: {other:?}"),
        }
    }

    fn number(n: usize) -> Address {
        Address {
            terms: vec![Term {
                base: Base::Number(n),
                offset: 0,
            }],
        }
    }

    #[test]
    fn tells_engine_commands_from_host_ones() {
        let cases = [
            ("s/a/b/", "Substitute"),
            (" :%substitute/a/b/", "Substitute"),
            ("2,3s", "Substitute"),
            ("s!a!b!", "Substitute"),
            ("noh", "ClearSearchHighlight"),
            ("nohlsearch", "ClearSearchHighlight"),
            ("3", "GoTo"),
            ("'a,/x/", "GoTo"),
            ("d", "Delete"),
            ("2,3y a", "Yank"),
            ("m0", "Move"),
            ("t.", "Copy"),
            ("co$", "Copy"),
            ("j!", "Join"),
            (">>", "Shift"),
            ("norm! x", "Normal"),
            ("g/a/d", "Global"),
            ("v/a/d", "Global"),
            ("sor u", "Sort"),
        ];
        for (line, expected) in cases {
            let parsed = command(line).unwrap();
            assert!(
                format!("{parsed:?}").starts_with(expected),
                "{line:?}: {parsed:?}"
            );
        }
        let host = |command: Command| Ok(Some(ParsedLine::Host(command.into())));
        assert_eq!(parse("wq"), host(Command::SaveAndQuit));
        assert_eq!(parse("sp"), host(Command::SplitDown));
        assert_eq!(parse("vs"), host(Command::SplitRight));
        assert_eq!(parse("  "), Ok(None));
    }

    #[test]
    fn reads_the_arguments() {
        assert_eq!(command("m 0"), Ok(ExCommand::Move(number(0))));
        assert_eq!(command("t 3"), Ok(ExCommand::Copy(number(3))));
        assert_eq!(
            command("<<< 2"),
            Ok(ExCommand::Shift {
                shift: Shift::Left,
                levels: 3,
                count: Some(2)
            })
        );
        assert_eq!(
            command("j 3"),
            Ok(ExCommand::Join {
                spaces: true,
                count: Some(3)
            })
        );
        assert_eq!(
            command("normal  Ax "),
            Ok(ExCommand::Normal("Ax ".to_owned()))
        );
        let ExCommand::Global(global) = command("g!/a/d").unwrap() else {
            panic!("no es :g");
        };
        assert!(global.inverted);
    }

    #[test]
    fn rejects_what_vim_rejects() {
        let cases = [
            ("sx/a/b/", CommandError::Unknown("sx/a/b/".to_owned())),
            ("no", CommandError::Unknown("no".to_owned())),
            ("3w", CommandError::PartialWrite),
            ("3zz", CommandError::Unknown("3zz".to_owned())),
            ("dd", CommandError::Unknown("dd".to_owned())),
            ("m", CommandError::InvalidRange),
            ("m x", CommandError::InvalidRange),
            ("d!", CommandError::NoBangAllowed),
            ("norm", CommandError::ArgumentRequired),
            ("1noh", CommandError::NoRangeAllowed),
            ("j 0", CommandError::PositiveCountRequired),
            ("g", CommandError::GlobalPatternMissing),
        ];
        for (line, error) in cases {
            assert_eq!(parse(line), Err(error), "{line:?}");
        }
    }
}
