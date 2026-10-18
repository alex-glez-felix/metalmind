//! Búsqueda del vault: la difusa de rutas del finder y la de texto completo
//! sobre las notas.

mod fuzzy;
mod text;

pub use fuzzy::{FuzzyMatcher, Query, Ranked};
pub use text::{
    Case, LineMatch, NoteMatches, SearchError, Syntax, TextMatcher, TextQuery, TextSearcher,
};
