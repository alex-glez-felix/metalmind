//! Comandos de `:` que el motor reconoce pero no ejecuta: guardar y cerrar
//! hacen I/O, así que los corre quien aloja el editor.

use crate::SearchError;

/// Un comando de la línea de comandos, invocable también por su nombre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// `:w`
    Save,
    /// `:q`, que se niega si hay cambios sin guardar.
    Quit,
    /// `:q!`
    ForceQuit,
    /// `:wq`
    SaveAndQuit,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CommandError {
    #[error("E492: no es un comando del editor: {0}")]
    Unknown(String),
    #[error("E16: rango inválido")]
    InvalidRange,
    #[error("E493: el rango va hacia atrás")]
    BackwardsRange,
    #[error("E488: sobra texto: {0}")]
    TrailingCharacters(String),
    #[error("nota no soporta el flag {0} de :s")]
    UnsupportedFlag(char),
    #[error("nota no soporta {0} en el reemplazo de :s")]
    UnsupportedReplacement(String),
    #[error(transparent)]
    Search(#[from] SearchError),
}

/// Nombre completo, cuántas letras bastan para abreviarlo y el comando.
const COMMANDS: &[(&str, usize, Command)] = &[
    ("write", 1, Command::Save),
    ("quit", 1, Command::Quit),
    ("quit!", 1, Command::ForceQuit),
    ("wq", 2, Command::SaveAndQuit),
];

impl Command {
    pub const ALL: &[Self] = &[Self::Save, Self::Quit, Self::ForceQuit, Self::SaveAndQuit];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Save => "editor::save",
            Self::Quit => "editor::quit",
            Self::ForceQuit => "editor::force_quit",
            Self::SaveAndQuit => "editor::save_and_quit",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|command| command.name() == name)
    }

    /// Interpreta lo escrito tras `:`. Como vim, ignora espacios y `:` de
    /// más y acepta abreviaturas (`:wri`, `:qu!`); una línea vacía no es nada.
    ///
    /// # Errors
    /// [`CommandError::Unknown`] si no es ningún comando conocido.
    pub fn parse(line: &str) -> Result<Option<Self>, CommandError> {
        let typed = line.trim_start_matches([' ', ':']).trim_end();
        if typed.is_empty() {
            return Ok(None);
        }
        COMMANDS
            .iter()
            .find(|&&(full, min, _)| abbreviates(typed, full, min))
            .map(|&(_, _, command)| Some(command))
            .ok_or_else(|| CommandError::Unknown(typed.to_owned()))
    }
}

/// `typed` es `full` recortado a `min` letras o más, conservando el `!`.
fn abbreviates(typed: &str, full: &str, min: usize) -> bool {
    let (typed, typed_bang) = split_bang(typed);
    let (full, full_bang) = split_bang(full);
    typed_bang == full_bang && typed.len() >= min && full.starts_with(typed)
}

fn split_bang(name: &str) -> (&str, bool) {
    name.strip_suffix('!')
        .map_or((name, false), |name| (name, true))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn every_command_round_trips_through_its_name() {
        for &command in Command::ALL {
            assert_eq!(Command::from_name(command.name()), Some(command));
        }
        let names: HashSet<_> = Command::ALL.iter().map(|c| c.name()).collect();
        assert_eq!(names.len(), Command::ALL.len());
    }

    #[test]
    fn parses_like_vim() {
        let cases = [
            ("w", Some(Command::Save)),
            ("wr", Some(Command::Save)),
            ("write", Some(Command::Save)),
            ("q", Some(Command::Quit)),
            ("quit", Some(Command::Quit)),
            ("q!", Some(Command::ForceQuit)),
            ("qui!", Some(Command::ForceQuit)),
            ("wq", Some(Command::SaveAndQuit)),
            ("  :w  ", Some(Command::Save)),
            ("::q", Some(Command::Quit)),
            ("", None),
            ("   ", None),
        ];
        for (line, expected) in cases {
            assert_eq!(Command::parse(line), Ok(expected), "{line:?}");
        }
    }

    #[test]
    fn rejects_what_it_does_not_know() {
        for line in ["x", "writes", "wq!", "w!", "q!!", "!", "e foo.md"] {
            assert_eq!(
                Command::parse(line),
                Err(CommandError::Unknown(line.to_owned())),
                "{line:?}"
            );
        }
    }
}
