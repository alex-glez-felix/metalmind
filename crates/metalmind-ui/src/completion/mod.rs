//! Las listas del autocompletado del editor, sin GPUI: la de `#` (tags) y
//! la de `[[` (notas), con la misma forma de elegir una fila.

mod links;
mod selection;
mod tags;

pub use self::links::{LinkCompletions, NoteCandidates};
pub use self::selection::Selection;
pub use self::tags::TagCompletions;
