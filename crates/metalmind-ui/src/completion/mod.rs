//! Las listas del autocompletado, sin GPUI: en el editor, la de `#` (tags)
//! y la de `[[` (notas); en la línea de `:`, la del historial, la del
//! nombre del comando y la del argumento que se escribe. Todas eligen una
//! fila igual.

mod argument;
mod command;
mod links;
mod paths;
mod selection;
mod tags;

pub use self::argument::{ArgumentCompletions, ArgumentList, FocusSources};
pub use self::command::CommandCompletions;
pub use self::links::{LinkCompletions, NoteCandidates};
pub use self::paths::{PathCandidates, PathCompletions};
pub use self::selection::Selection;
pub use self::tags::TagCompletions;
