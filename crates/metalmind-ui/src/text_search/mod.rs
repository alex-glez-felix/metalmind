//! `space /`: buscar texto en todas las notas del vault, como el live grep
//! de Telescope. Es el picker genérico con un delegado que busca con el
//! `TextSearcher` de `metalmind-search` en un hilo propio; cada fila es una
//! línea que coincide, con su nota y su número, y elegirla abre la nota en
//! esa línea.

mod context;
mod delegate;
mod model;
mod worker;

use std::fs;
use std::path::Path;

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

/// Si una nota abierta con cambios, la de `path` en el vault y `file` en el
/// disco, se busca en memoria: las notas del índice, que son las que recorre
/// la búsqueda en disco, y las que no están en el disco ahora (nuevas sin
/// guardar, o borradas o movidas desde fuera). Lo que está en disco y el
/// índice no tiene (oculto o ignorado) o no es una nota, no.
#[must_use]
pub fn searchable(index: &VaultIndex, path: &VaultPath, file: &Path) -> bool {
    match index.kind(path) {
        Some(kind) => kind == EntryKind::Note,
        None => EntryKind::of_file(path) == EntryKind::Note && fs::symlink_metadata(file).is_err(),
    }
}

#[cfg(test)]
mod tests {
    use metalmind_vault::Vault;

    use super::*;

    #[test]
    fn se_buscan_las_notas_del_indice_y_las_que_no_estan_en_disco() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["a.md", "datos.txt", "ignorada.md", "borrada.md"] {
            fs::write(dir.path().join(name), "").unwrap();
        }
        fs::write(dir.path().join(".gitignore"), "ignorada.md\n").unwrap();
        fs::remove_file(dir.path().join("borrada.md")).unwrap();
        let (vault, _updates) = Vault::open(dir.path()).unwrap();
        let index = vault.snapshot();
        let searchable = |name: &str| {
            let path = VaultPath::new(name).unwrap();
            searchable(&index, &path, &dir.path().join(name))
        };
        assert!(searchable("a.md"));
        assert!(searchable("nueva.md"), "sin guardar");
        assert!(searchable("d/Nueva.MD"));
        assert!(searchable("borrada.md"), "borrada desde fuera");
        assert!(!searchable("datos.txt"));
        assert!(!searchable("nuevo.txt"));
        assert!(!searchable("ignorada.md"), "ignorada");
    }
}
