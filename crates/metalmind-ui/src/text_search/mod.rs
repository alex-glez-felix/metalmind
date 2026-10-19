//! `space /`: buscar texto en todas las notas del vault, como el live grep
//! de Telescope. Es el picker genérico con un delegado que busca con el
//! `TextSearcher` de `metalmind-search` en un hilo propio; cada fila es una
//! línea que coincide, con su nota y su número, y elegirla abre la nota en
//! esa línea.

mod context;
mod delegate;
mod model;
mod worker;

use metalmind_buffer::Snapshot;
use metalmind_search::LineMatch;
use metalmind_vault::VaultPath;

pub use self::context::{RowText, row_text};
pub(crate) use self::delegate::TextSearch;
pub use self::model::{MAX_HITS, Search, Status, TextSearchState};
pub use self::worker::{Event, SearchWorker};

/// Una línea que coincide, en su nota.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    pub path: VaultPath,
    pub line: LineMatch,
}

impl Hit {
    /// Dónde va el cursor: la primera coincidencia de la línea, en bytes.
    #[must_use]
    pub fn column(&self) -> usize {
        self.line.matches.first().map_or(0, |found| found.start)
    }
}

/// Una nota abierta con cambios sin guardar: se busca su texto en memoria,
/// que es el que se ve, y no el del disco.
#[derive(Clone, Debug)]
pub struct OpenNote {
    pub path: VaultPath,
    pub text: Snapshot,
}
