//! Las acciones que el motor no conoce: los atajos y los comandos de `:`
//! que quien aloja el editor le declara (guardar, abrir, los paneles). El
//! motor las reconoce y devuelve su nombre sin saber qué hacen.

use crate::SearchError;

/// El nombre de una acción ajena al motor, como `workspace::open_finder`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ActionName(&'static str);

impl ActionName {
    #[must_use]
    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl std::fmt::Display for ActionName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// Una acción ajena con el argumento que se le escribió, como `:e notas/a.md`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invocation {
    pub action: ActionName,
    /// Lo escrito tras el nombre, sin espacios alrededor; solo lo aceptan
    /// los comandos con [`HostCommand::takes_argument`].
    pub argument: Option<String>,
}

impl From<ActionName> for Invocation {
    fn from(action: ActionName) -> Self {
        Self {
            action,
            argument: None,
        }
    }
}

/// Un comando de `:` que corre quien aloja el editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostCommand {
    /// El nombre completo, con su `!` si lo lleva: `write!`.
    pub name: &'static str,
    /// Cuántas letras bastan para abreviarlo, como en vim.
    pub min: usize,
    pub action: ActionName,
    pub takes_argument: bool,
    /// Qué error da con un rango, que ninguno admite.
    pub with_range: RangeRule,
}

/// Por qué un comando ajeno no corre con rango.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeRule {
    /// Con rango, vim escribiría ese tramo, y Metalmind no escribe tramos.
    PartialWrite,
    /// Vim tampoco lo admite.
    NotAllowed,
    /// En vim el rango es un número de ventana o un alto, que Metalmind no
    /// tiene.
    Unsupported,
}

impl HostCommand {
    pub(crate) fn range_error(&self, typed: &str) -> CommandError {
        match self.with_range {
            RangeRule::PartialWrite => CommandError::PartialWrite,
            RangeRule::NotAllowed => CommandError::NoRangeAllowed,
            RangeRule::Unsupported => CommandError::UnsupportedRange(typed.to_owned()),
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
    #[error("El cursor no está en una tabla")]
    NotInTable,
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

/// Lo escrito tras `:` cuando es uno de los comandos ajenos.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Parsed<'a> {
    pub command: &'a HostCommand,
    pub argument: Option<String>,
}

impl Parsed<'_> {
    pub fn invocation(self) -> Invocation {
        Invocation {
            action: self.command.action,
            argument: self.argument,
        }
    }
}

/// Interpreta lo escrito tras `:` con los comandos de `commands`. Como
/// vim, ignora espacios y `:` de más y acepta abreviaturas (`:wri`, `:qu!`,
/// `:e!nota.md`); una línea vacía no es nada. El argumento va tal cual: no
/// se expanden `%`, `#` ni `~` y los espacios no se escapan.
///
/// # Errors
/// [`CommandError::Unknown`] si no es ningún comando conocido y
/// [`CommandError::TrailingCharacters`] si lleva un argumento que no
/// acepta.
pub(crate) fn parse<'a>(
    line: &str,
    commands: &'a [HostCommand],
) -> Result<Option<Parsed<'a>>, CommandError> {
    let typed = line.trim_start_matches([' ', ':']).trim_end();
    if typed.is_empty() {
        return Ok(None);
    }
    let (name, rest) = split_name(typed);
    let command = find(name, commands).ok_or_else(|| CommandError::Unknown(typed.to_owned()))?;
    let argument = rest.trim_start();
    if argument.is_empty() {
        return Ok(Some(Parsed {
            command,
            argument: None,
        }));
    }
    if !command.takes_argument {
        return Err(CommandError::TrailingCharacters(argument.to_owned()));
    }
    Ok(Some(Parsed {
        command,
        argument: Some(argument.to_owned()),
    }))
}

/// El comando ajeno de `line` que acepta argumento y dónde empieza este,
/// en bytes, si ya se escribió el espacio tras el nombre: `:focus #ru` da
/// `focus` y 7. Como [`parse`], ignora espacios y `:` de más.
pub(crate) fn argument_start<'a>(
    line: &str,
    commands: &'a [HostCommand],
) -> Option<(&'a HostCommand, usize)> {
    let typed = line.trim_start_matches([' ', ':']);
    let (name, rest) = split_name(typed);
    let command = find(name, commands).filter(|command| command.takes_argument)?;
    let argument = rest.trim_start();
    (argument.len() < rest.len()).then(|| (command, line.len() - argument.len()))
}

fn find<'a>(name: &str, commands: &'a [HostCommand]) -> Option<&'a HostCommand> {
    commands
        .iter()
        .find(|command| abbreviates(name, command.name, command.min))
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
pub(crate) fn abbreviates(typed: &str, full: &str, min: usize) -> bool {
    let (typed, typed_bang) = split_bang(typed);
    let (full, full_bang) = split_bang(full);
    typed_bang == full_bang && typed.len() >= min && full.starts_with(typed)
}

pub(crate) fn split_bang(name: &str) -> (&str, bool) {
    name.strip_suffix('!')
        .map_or((name, false), |name| (name, true))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAVE: ActionName = ActionName::new("editor::save");
    const QUIT: ActionName = ActionName::new("editor::quit");

    const COMMANDS: &[HostCommand] = &[
        HostCommand {
            name: "write",
            min: 1,
            action: SAVE,
            takes_argument: true,
            with_range: RangeRule::PartialWrite,
        },
        HostCommand {
            name: "quit",
            min: 1,
            action: QUIT,
            takes_argument: false,
            with_range: RangeRule::Unsupported,
        },
    ];

    fn invocation(line: &str) -> Result<Option<Invocation>, CommandError> {
        parse(line, COMMANDS).map(|parsed| parsed.map(Parsed::invocation))
    }

    #[test]
    fn abrevia_como_vim() {
        for line in ["w", "wr", "write", "  :w  ", "::w"] {
            assert_eq!(invocation(line), Ok(Some(SAVE.into())), "{line:?}");
        }
        for line in ["writes", "w!", "q!", "x"] {
            assert_eq!(
                invocation(line),
                Err(CommandError::Unknown(line.to_owned())),
                "{line:?}"
            );
        }
        assert_eq!(invocation("   "), Ok(None));
    }

    #[test]
    fn el_argumento_es_el_resto_de_la_linea() {
        assert_eq!(
            invocation("w   notas/b c.md  "),
            Ok(Some(Invocation {
                action: SAVE,
                argument: Some("notas/b c.md".to_owned()),
            }))
        );
        assert_eq!(
            invocation("q otro.md"),
            Err(CommandError::TrailingCharacters("otro.md".to_owned()))
        );
    }

    #[test]
    fn el_argumento_empieza_tras_el_nombre_y_un_espacio() {
        let start =
            |line| argument_start(line, COMMANDS).map(|(command, start)| (command.name, start));
        assert_eq!(start("w notas/a"), Some(("write", 2)));
        assert_eq!(start(" :wr   notas/a"), Some(("write", 7)));
        assert_eq!(start("w "), Some(("write", 2)));
        assert_eq!(start("w"), None);
        assert_eq!(start("q x"), None);
        assert_eq!(start("x a"), None);
    }

    #[test]
    fn con_rango_da_el_error_de_su_regla() {
        let error = |line| {
            parse(line, COMMANDS)
                .unwrap()
                .unwrap()
                .command
                .range_error(line)
        };
        assert_eq!(error("w"), CommandError::PartialWrite);
        assert_eq!(error("qu"), CommandError::UnsupportedRange("qu".to_owned()));
    }
}
