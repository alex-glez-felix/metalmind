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
use metalmind_vault::{EntryKind, VaultIndex, VaultPath};

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

/// Si una nota abierta con cambios se busca en memoria: las notas del índice,
/// que son las que recorre la búsqueda en disco, y las nuevas que todavía no
/// se guardaron. Lo que está en disco y el índice no tiene (oculto o
/// ignorado) o no es una nota, no.
#[must_use]
pub fn searchable(index: &VaultIndex, path: &VaultPath, on_disk: bool) -> bool {
    match index.kind(path) {
        Some(kind) => kind == EntryKind::Note,
        None => !on_disk && EntryKind::of_file(path) == EntryKind::Note,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use metalmind_vault::Vault;

    use super::*;

    #[test]
    fn se_buscan_las_notas_del_indice_y_las_nuevas_sin_guardar() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.md"), "").unwrap();
        fs::write(dir.path().join("datos.txt"), "").unwrap();
        fs::write(dir.path().join(".gitignore"), "ignorada.md\n").unwrap();
        fs::write(dir.path().join("ignorada.md"), "").unwrap();
        let (vault, _updates) = Vault::open(dir.path()).unwrap();
        let index = vault.snapshot();
        let path = |text: &str| VaultPath::new(text).unwrap();
        assert!(searchable(&index, &path("a.md"), true));
        assert!(searchable(&index, &path("nueva.md"), false), "sin guardar");
        assert!(searchable(&index, &path("d/Nueva.MD"), false));
        assert!(!searchable(&index, &path("datos.txt"), true));
        assert!(!searchable(&index, &path("nuevo.txt"), false));
        assert!(!searchable(&index, &path("ignorada.md"), true), "ignorada");
    }
}
