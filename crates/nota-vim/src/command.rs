//! Comandos de `:` y de `ctrl-w` que el motor reconoce pero no ejecuta:
//! guardar, abrir y cerrar hacen I/O y los paneles son de quien aloja el
//! editor.

use crate::SearchError;

/// Un comando de la línea de comandos, invocable también por su nombre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// `:w`
    Save,
    /// `:q`, que cierra el panel y se niega si se perderían cambios.
    Quit,
    /// `:q!`
    ForceQuit,
    /// `:wq`
    SaveAndQuit,
    /// `:e [ruta]`, que se niega si se perderían cambios. Sin ruta, relee
    /// el archivo del panel.
    Edit,
    /// `:e! [ruta]`, que descarta los cambios.
    ForceEdit,
    /// `ctrl-w v`, `:vsplit`
    SplitRight,
    /// `ctrl-w s`, `:split`
    SplitDown,
    /// `ctrl-w h`
    FocusLeft,
    /// `ctrl-w j`
    FocusDown,
    /// `ctrl-w k`
    FocusUp,
    /// `ctrl-w l`
    FocusRight,
    /// `ctrl-w w`
    FocusNext,
    /// `ctrl-w o`, `:only`
    Only,
    /// `:set {opciones}`: las opciones son de quien aloja el editor, como
    /// los números de línea de cada panel.
    Set,
    /// `space f`: el finder de notas.
    OpenFinder,
    /// `space p`: la paleta de comandos.
    OpenCommandPalette,
}

/// Un comando con el argumento que se le escribió, como `:e notas/a.md`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invocation {
    pub command: Command,
    /// Lo escrito tras el nombre, sin espacios alrededor; solo lo aceptan
    /// los comandos con [`Command::takes_argument`].
    pub argument: Option<String>,
}

impl From<Command> for Invocation {
    fn from(command: Command) -> Self {
        Self {
            command,
            argument: None,
        }
    }
}

/// Por qué no se pudo correr lo escrito tras `:`. Los errores de vim llevan
/// su número y su texto original; los de nota, que vim no tiene, van en
/// español.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CommandError {
    #[error("E492: Not an editor command: {0}")]
    Unknown(String),
    #[error("E16: Invalid range")]
    InvalidRange,
    #[error("E493: Backwards range given")]
    BackwardsRange,
    #[error("E488: Trailing characters: {0}")]
    TrailingCharacters(String),
    #[error("E20: Mark not set")]
    MarkNotSet,
    #[error("E14: Invalid address")]
    InvalidAddress,
    #[error("E134: Cannot move a range of lines into itself")]
    MoveIntoItself,
    #[error("E471: Argument required")]
    ArgumentRequired,
    #[error("E474: Invalid argument")]
    InvalidArgument,
    #[error("E477: No ! allowed")]
    NoBangAllowed,
    #[error("E481: No range allowed")]
    NoRangeAllowed,
    #[error("E654: Missing delimiter after search pattern: {0}")]
    MissingDelimiter(String),
    #[error("E939: Positive count required")]
    PositiveCountRequired,
    #[error("E146: Regular expressions can't be delimited by letters")]
    LetterDelimiter,
    #[error("E147: Cannot do :global recursive with a range")]
    RecursiveGlobal,
    #[error("E148: Regular expression missing from :global")]
    GlobalPatternMissing,
    #[error("nota no soporta el flag {0} de :s")]
    UnsupportedFlag(char),
    #[error("nota no soporta {0} en el reemplazo de :s")]
    UnsupportedReplacement(String),
    #[error("nota no soporta el flag {0} de :sort")]
    UnsupportedSortFlag(char),
    /// `:g` y `:normal` corren dentro del motor; un comando que tiene que
    /// correr quien aloja el editor no puede ir dentro.
    #[error("nota no corre :{0} dentro de :g ni de :normal")]
    HostCommandInside(String),
    #[error(transparent)]
    Search(#[from] SearchError),
}

/// Nombre completo, cuántas letras bastan para abreviarlo y el comando.
const COMMANDS: &[(&str, usize, Command)] = &[
    ("write", 1, Command::Save),
    ("quit", 1, Command::Quit),
    ("quit!", 1, Command::ForceQuit),
    ("wq", 2, Command::SaveAndQuit),
    ("edit", 1, Command::Edit),
    ("edit!", 1, Command::ForceEdit),
    ("split", 2, Command::SplitDown),
    ("vsplit", 2, Command::SplitRight),
    ("only", 2, Command::Only),
    ("set", 2, Command::Set),
];

impl Command {
    pub const ALL: &[Self] = &[
        Self::Save,
        Self::Quit,
        Self::ForceQuit,
        Self::SaveAndQuit,
        Self::Edit,
        Self::ForceEdit,
        Self::SplitRight,
        Self::SplitDown,
        Self::FocusLeft,
        Self::FocusDown,
        Self::FocusUp,
        Self::FocusRight,
        Self::FocusNext,
        Self::Only,
        Self::Set,
        Self::OpenFinder,
        Self::OpenCommandPalette,
    ];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Save => "editor::save",
            Self::Quit => "editor::quit",
            Self::ForceQuit => "editor::force_quit",
            Self::SaveAndQuit => "editor::save_and_quit",
            Self::Edit => "editor::edit",
            Self::ForceEdit => "editor::force_edit",
            Self::SplitRight => "pane::split_right",
            Self::SplitDown => "pane::split_down",
            Self::FocusLeft => "pane::focus_left",
            Self::FocusDown => "pane::focus_down",
            Self::FocusUp => "pane::focus_up",
            Self::FocusRight => "pane::focus_right",
            Self::FocusNext => "pane::focus_next",
            Self::Only => "pane::only",
            Self::Set => "editor::set",
            Self::OpenFinder => "workspace::open_finder",
            Self::OpenCommandPalette => "workspace::open_command_palette",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|command| command.name() == name)
    }

    #[must_use]
    pub fn takes_argument(self) -> bool {
        matches!(self, Self::Edit | Self::ForceEdit | Self::Set)
    }
}

impl Invocation {
    /// Interpreta lo escrito tras `:`. Como vim, ignora espacios y `:` de
    /// más y acepta abreviaturas (`:wri`, `:qu!`, `:e!nota.md`); una línea
    /// vacía no es nada. El argumento va tal cual: no se expanden `%`, `#`
    /// ni `~` y los espacios no se escapan.
    ///
    /// # Errors
    /// [`CommandError::Unknown`] si no es ningún comando conocido y
    /// [`CommandError::TrailingCharacters`] si lleva un argumento que no
    /// acepta.
    pub fn parse(line: &str) -> Result<Option<Self>, CommandError> {
        let typed = line.trim_start_matches([' ', ':']).trim_end();
        if typed.is_empty() {
            return Ok(None);
        }
        let (name, rest) = split_name(typed);
        let command = COMMANDS
            .iter()
            .find(|&&(full, min, _)| abbreviates(name, full, min))
            .map(|&(_, _, command)| command)
            .ok_or_else(|| CommandError::Unknown(typed.to_owned()))?;
        let argument = rest.trim_start();
        if argument.is_empty() {
            return Ok(Some(command.into()));
        }
        if !command.takes_argument() {
            return Err(CommandError::TrailingCharacters(argument.to_owned()));
        }
        Ok(Some(Self {
            command,
            argument: Some(argument.to_owned()),
        }))
    }
}

/// El nombre son las letras del principio con su `!`, si lo lleva; lo
/// demás es el argumento.
fn split_name(typed: &str) -> (&str, &str) {
    let letters = typed
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(typed.len());
    let end = if typed[letters..].starts_with('!') {
        letters + 1
    } else {
        letters
    };
    typed.split_at(end)
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
            ("e", Some(Command::Edit)),
            ("edit!", Some(Command::ForceEdit)),
            ("sp", Some(Command::SplitDown)),
            ("vs", Some(Command::SplitRight)),
            ("vsplit", Some(Command::SplitRight)),
            ("on", Some(Command::Only)),
            ("", None),
            ("   ", None),
        ];
        for (line, expected) in cases {
            assert_eq!(
                Invocation::parse(line),
                Ok(expected.map(Invocation::from)),
                "{line:?}"
            );
        }
    }

    #[test]
    fn edit_and_set_take_the_rest_of_the_line() {
        let cases = [
            ("e a.md", Command::Edit, "a.md"),
            ("edit   notas/b c.md  ", Command::Edit, "notas/b c.md"),
            ("e! ../a.md", Command::ForceEdit, "../a.md"),
            ("e!a.md", Command::ForceEdit, "a.md"),
            ("set nu", Command::Set, "nu"),
            ("se nonu rnu!", Command::Set, "nonu rnu!"),
        ];
        for (line, command, path) in cases {
            assert_eq!(
                Invocation::parse(line),
                Ok(Some(Invocation {
                    command,
                    argument: Some(path.to_owned()),
                })),
                "{line:?}"
            );
        }
    }

    #[test]
    fn rejects_what_it_does_not_know() {
        for line in ["x", "writes", "wq!", "w!", "!", "efoo", "s", "v", "o"] {
            assert_eq!(
                Invocation::parse(line),
                Err(CommandError::Unknown(line.to_owned())),
                "{line:?}"
            );
        }
    }

    #[test]
    fn rejects_an_argument_it_does_not_take() {
        let cases = [("q!!", "!"), ("w otro.md", "otro.md"), ("vs a.md", "a.md")];
        for (line, rest) in cases {
            assert_eq!(
                Invocation::parse(line),
                Err(CommandError::TrailingCharacters(rest.to_owned())),
                "{line:?}"
            );
        }
    }
}
