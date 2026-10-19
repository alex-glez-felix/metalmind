//! Comandos de `:`, de `ctrl-w` y de `space` que el motor reconoce pero no
//! ejecuta: guardar, abrir y cerrar hacen I/O, y los paneles y el panel
//! lateral son de quien aloja el editor.

use crate::SearchError;

/// Un comando de la línea de comandos, invocable también por su nombre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// `:w [ruta]`; con ruta, le da archivo a un buffer sin nombre o
    /// escribe una copia.
    Save,
    /// `:w! [ruta]`: guarda aunque el archivo haya cambiado en disco desde
    /// que se leyó, y con ruta pisa un archivo que ya existe.
    ForceSave,
    /// `:q`, que cierra el panel; en el último, se niega si algún buffer
    /// tiene cambios.
    Quit,
    /// `:q!`
    ForceQuit,
    /// `:wq [ruta]`
    SaveAndQuit,
    /// `:wa`, que guarda todos los buffers con cambios.
    SaveAll,
    /// `:qa`, que cierra la ventana si ningún buffer tiene cambios.
    QuitAll,
    /// `:qa!`, que cierra la ventana aunque haya cambios.
    ForceQuitAll,
    /// `:wqa` y `:xa`: guarda todos los buffers con cambios y cierra.
    SaveAllAndQuit,
    /// `:e [ruta]`. Con otra ruta, el buffer de antes queda oculto con sus
    /// cambios, como con `hidden` de vim; sin ruta, relee el archivo del
    /// panel y se niega si se perderían cambios.
    Edit,
    /// `:e! [ruta]`; sin ruta, relee el archivo y descarta los cambios.
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
    /// `space /`: la búsqueda de texto en las notas del vault.
    OpenTextSearch,
    /// `space p`: la paleta de comandos.
    OpenCommandPalette,
    /// `space e`: muestra el panel lateral en la vista de carpetas, o lo
    /// esconde si ya la muestra.
    ToggleFolders,
    /// `space t`: lo mismo con la vista de tags.
    ToggleTags,
    /// `gd` o `enter`: abre lo que nombra el link bajo el cursor.
    FollowLink,
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
/// su número y su texto original; los de Metalmind, que vim no tiene, van en
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
    #[error("E30: No previous command line")]
    NoPreviousCommandLine,
    #[error("E134: Cannot move a range of lines into itself")]
    MoveIntoItself,
    #[error("E140: Use ! to write partial buffer")]
    PartialWrite,
    #[error("E169: Command too recursive")]
    TooRecursive,
    #[error("E471: Argument required")]
    ArgumentRequired,
    #[error("E474: Invalid argument")]
    InvalidArgument,
    /// Con lo que queda de la línea desde lo que no se entendió.
    #[error("E475: Invalid argument: {0}")]
    InvalidArgumentAt(String),
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
    #[error("E148: Regular expression missing from global")]
    GlobalPatternMissing,
    #[error("Metalmind no soporta el flag {0} de :s")]
    UnsupportedFlag(char),
    #[error("Metalmind no soporta {0} en el reemplazo de :s")]
    UnsupportedReplacement(String),
    #[error("Metalmind no soporta el flag {0} de :sort")]
    UnsupportedSortFlag(char),
    /// `'<` y `'>` son la última selección de Visual, que en Metalmind no se
    /// borra de a un extremo.
    #[error("Metalmind no borra la mark {0} con :delm")]
    UnsupportedDeleteMark(char),
    /// En vim, el rango de `:q`, `:sp`, `:vs` y `:on` es un número de
    /// ventana o un alto, que Metalmind no tiene.
    #[error("Metalmind no admite un rango en :{0}")]
    UnsupportedRange(String),
    /// `:g` y `:normal` corren dentro del motor; un comando que tiene que
    /// correr quien aloja el editor no puede ir dentro.
    #[error("Metalmind no corre :{0} dentro de :g ni de :normal")]
    HostCommandInside(String),
    /// Un salto a otro archivo (`ctrl-o`, `'A`) dentro de `:g` o `:normal`.
    #[error("Metalmind no cambia de archivo dentro de :g ni de :normal")]
    FileSwitchInside,
    #[error(transparent)]
    Search(#[from] SearchError),
}

/// Nombre completo, cuántas letras bastan para abreviarlo y el comando.
const COMMANDS: &[(&str, usize, Command)] = &[
    ("write", 1, Command::Save),
    ("write!", 1, Command::ForceSave),
    ("quit", 1, Command::Quit),
    ("quit!", 1, Command::ForceQuit),
    ("wq", 2, Command::SaveAndQuit),
    ("wall", 2, Command::SaveAll),
    ("qall", 2, Command::QuitAll),
    ("quitall", 5, Command::QuitAll),
    ("qall!", 2, Command::ForceQuitAll),
    ("quitall!", 5, Command::ForceQuitAll),
    ("wqall", 3, Command::SaveAllAndQuit),
    ("xall", 2, Command::SaveAllAndQuit),
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
        Self::ForceSave,
        Self::Quit,
        Self::ForceQuit,
        Self::SaveAndQuit,
        Self::SaveAll,
        Self::QuitAll,
        Self::ForceQuitAll,
        Self::SaveAllAndQuit,
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
        Self::OpenTextSearch,
        Self::OpenCommandPalette,
        Self::ToggleFolders,
        Self::ToggleTags,
        Self::FollowLink,
    ];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Save => "editor::save",
            Self::ForceSave => "editor::force_save",
            Self::Quit => "editor::quit",
            Self::ForceQuit => "editor::force_quit",
            Self::SaveAndQuit => "editor::save_and_quit",
            Self::SaveAll => "workspace::save_all",
            Self::QuitAll => "workspace::quit_all",
            Self::ForceQuitAll => "workspace::force_quit_all",
            Self::SaveAllAndQuit => "workspace::save_all_and_quit",
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
            Self::OpenTextSearch => "workspace::open_text_search",
            Self::OpenCommandPalette => "workspace::open_command_palette",
            Self::ToggleFolders => "panel::toggle_folders",
            Self::ToggleTags => "panel::toggle_tags",
            Self::FollowLink => "editor::follow_link",
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
        matches!(
            self,
            Self::Save
                | Self::ForceSave
                | Self::SaveAndQuit
                | Self::Edit
                | Self::ForceEdit
                | Self::Set
        )
    }

    /// Por qué no corre con un rango, que ningún comando del host admite.
    pub(crate) fn range_error(self, typed: &str) -> CommandError {
        match self {
            Self::Save | Self::ForceSave | Self::SaveAndQuit => CommandError::PartialWrite,
            Self::SaveAll | Self::QuitAll | Self::ForceQuitAll | Self::SaveAllAndQuit => {
                CommandError::NoRangeAllowed
            }
            Self::Edit | Self::ForceEdit | Self::Set => CommandError::NoRangeAllowed,
            _ => CommandError::UnsupportedRange(typed.to_owned()),
        }
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
            ("w!", Some(Command::ForceSave)),
            ("write!", Some(Command::ForceSave)),
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
            ("wa", Some(Command::SaveAll)),
            ("wall", Some(Command::SaveAll)),
            ("qa", Some(Command::QuitAll)),
            ("qall", Some(Command::QuitAll)),
            ("quita", Some(Command::QuitAll)),
            ("quitall", Some(Command::QuitAll)),
            ("qa!", Some(Command::ForceQuitAll)),
            ("quitall!", Some(Command::ForceQuitAll)),
            ("wqa", Some(Command::SaveAllAndQuit)),
            ("wqall", Some(Command::SaveAllAndQuit)),
            ("xa", Some(Command::SaveAllAndQuit)),
            ("xall", Some(Command::SaveAllAndQuit)),
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
            ("w nueva.md", Command::Save, "nueva.md"),
            ("w! nueva.md", Command::ForceSave, "nueva.md"),
            ("wq notas/b.md", Command::SaveAndQuit, "notas/b.md"),
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
        for line in ["x", "writes", "wq!", "!", "efoo", "s", "v", "o", "qx", "xb"] {
            assert_eq!(
                Invocation::parse(line),
                Err(CommandError::Unknown(line.to_owned())),
                "{line:?}"
            );
        }
    }

    #[test]
    fn rejects_an_argument_it_does_not_take() {
        let cases = [
            ("q!!", "!"),
            ("q otro.md", "otro.md"),
            ("vs a.md", "a.md"),
            ("wa otro.md", "otro.md"),
            ("qa!x", "x"),
        ];
        for (line, rest) in cases {
            assert_eq!(
                Invocation::parse(line),
                Err(CommandError::TrailingCharacters(rest.to_owned())),
                "{line:?}"
            );
        }
    }
}
