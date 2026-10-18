//! Un vault en disco para los tests: el `TagGraph` solo sale de uno.

use std::fs;

use metalmind_vault::Vault;
use metalmind_vault::tags::TagGraph;

/// El grafo de un vault con estos archivos.
pub(crate) fn graph(files: &[(&str, &str)]) -> TagGraph {
    let dir = tempfile::tempdir().unwrap();
    for (path, text) in files {
        let full = dir.path().join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, text).unwrap();
    }
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    vault.tags().graph().clone()
}
