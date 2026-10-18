//! Búsqueda del vault: la difusa de rutas del finder y la de texto completo
//! sobre las notas.

mod fuzzy;
mod text;

pub use fuzzy::{PathMatch, PathMatcher};
pub use text::{Case, LineMatch, NoteMatches, SearchError, Syntax, TextQuery, TextSearcher};
