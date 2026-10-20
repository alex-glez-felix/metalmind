//! Qué notas cubre la búsqueda de texto, para quien busca también en las
//! que tiene abiertas con cambios.

use std::fs;

use crate::index::EntryKind;
use crate::path::VaultPath;
use crate::vault::Vault;

impl Vault {
    /// Si la búsqueda de texto cubre la nota `path`: las notas del índice,
    /// que son las que recorre [`crate::walk_notes`], y las que no están en
    /// el disco ahora (nuevas sin guardar, o borradas o movidas desde fuera).
    /// Lo que está en disco y el índice no tiene (oculto o ignorado) o no es
    /// una nota, no.
    #[must_use]
    pub fn is_searchable(&self, path: &VaultPath) -> bool {
        match self.snapshot().kind(path) {
            Some(kind) => kind == EntryKind::Note,
            None => {
                EntryKind::of_file(path) == EntryKind::Note
                    && fs::symlink_metadata(path.to_fs(self.root())).is_err()
            }
        }
    }
}

#[cfg(test)]
mod tests {
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
        let searchable = |name: &str| vault.is_searchable(&VaultPath::new(name).unwrap());
        assert!(searchable("a.md"));
        assert!(searchable("nueva.md"), "sin guardar");
        assert!(searchable("d/Nueva.MD"));
        assert!(searchable("borrada.md"), "borrada desde fuera");
        assert!(!searchable("datos.txt"));
        assert!(!searchable("nuevo.txt"));
        assert!(!searchable("ignorada.md"), "ignorada");
    }
}
