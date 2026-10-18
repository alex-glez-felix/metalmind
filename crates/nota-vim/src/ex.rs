//! Lo escrito tras `:`: los comandos que corre el motor (`:s`, `:noh`) y los
//! que corre quien aloja el editor ([`Command`]).

use crate::range::LineRange;
use crate::substitute::SubstituteCommand;
use crate::{Command, CommandError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExCommand {
    Host(Command),
    Substitute(SubstituteCommand),
    /// `:noh`: deja de resaltar la búsqueda hasta la próxima.
    ClearSearchHighlight,
}

impl ExCommand {
    /// Como vim, ignora espacios y `:` de más; una línea vacía no es nada.
    ///
    /// # Errors
    /// Si no es un comando conocido o sus argumentos no valen.
    pub fn parse(line: &str) -> Result<Option<Self>, CommandError> {
        let typed = line.trim_start_matches([' ', ':']);
        if typed.trim_end().is_empty() {
            return Ok(None);
        }
        let (range, rest) = LineRange::parse(typed)?;
        let name_len = rest
            .find(|c: char| !c.is_ascii_alphabetic())
            .unwrap_or(rest.len());
        let (name, args) = rest.split_at(name_len);
        if !name.is_empty() && "substitute".starts_with(name) {
            return SubstituteCommand::parse(range, args).map(|s| Some(Self::Substitute(s)));
        }
        if range.is_none()
            && name.len() >= 3
            && "nohlsearch".starts_with(name)
            && args.trim().is_empty()
        {
            return Ok(Some(Self::ClearSearchHighlight));
        }
        Command::parse(line).map(|command| command.map(Self::Host))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tells_engine_commands_from_host_ones() {
        assert!(matches!(
            ExCommand::parse("s/a/b/"),
            Ok(Some(ExCommand::Substitute(_)))
        ));
        assert!(matches!(
            ExCommand::parse(" :%substitute/a/b/"),
            Ok(Some(ExCommand::Substitute(_)))
        ));
        assert!(matches!(
            ExCommand::parse("2,3s"),
            Ok(Some(ExCommand::Substitute(_)))
        ));
        assert_eq!(
            ExCommand::parse("noh"),
            Ok(Some(ExCommand::ClearSearchHighlight))
        );
        assert_eq!(
            ExCommand::parse("nohlsearch"),
            Ok(Some(ExCommand::ClearSearchHighlight))
        );
        assert_eq!(
            ExCommand::parse("wq"),
            Ok(Some(ExCommand::Host(Command::SaveAndQuit)))
        );
        assert_eq!(ExCommand::parse("  "), Ok(None));
    }

    #[test]
    fn does_not_take_other_commands_for_substitute() {
        for line in ["sp", "sx/a/b/", "no", "3w"] {
            assert_eq!(
                ExCommand::parse(line),
                Err(CommandError::Unknown(line.to_owned())),
                "{line:?}"
            );
        }
    }
}
