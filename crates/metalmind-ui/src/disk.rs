//! Dónde escribe un documento al guardarse: una nota del vault pasa por
//! [`Vault::save`], que registra su versión en el historial (06 §3); lo de
//! fuera del vault se escribe a secas. Las dos escrituras son atómicas.

use std::fmt;
use std::path::Path;
use std::rc::Rc;

use metalmind_vault::{EntryKind, SaveError, SaveKind, Vault};

use crate::finder::{canonical, relative_to};

pub trait Disk: fmt::Debug {
    /// Escribe `text` en `path` de forma atómica.
    ///
    /// # Errors
    /// Si no se pudo escribir; el archivo queda como estaba.
    fn write(&self, path: &Path, text: &str, kind: SaveKind) -> Result<(), SaveError>;
}

/// El disco sin vault ni historial.
#[derive(Debug, Default)]
pub struct Files;

impl Disk for Files {
    fn write(&self, path: &Path, text: &str, _: SaveKind) -> Result<(), SaveError> {
        metalmind_vault::write_atomic(path, |out| out.write_all(text.as_bytes()))?;
        Ok(())
    }
}

/// Las notas del vault de la ventana, con su historial.
pub struct VaultDisk(Rc<Vault>);

impl VaultDisk {
    #[must_use]
    pub fn new(vault: Rc<Vault>) -> Self {
        Self(vault)
    }
}

impl fmt::Debug for VaultDisk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("VaultDisk").field(&self.0.root()).finish()
    }
}

impl Disk for VaultDisk {
    fn write(&self, path: &Path, text: &str, kind: SaveKind) -> Result<(), SaveError> {
        let note = canonical(path)
            .and_then(|path| relative_to(self.0.root(), &path))
            .filter(|note| EntryKind::of_file(note) == EntryKind::Note);
        match note {
            // La versión se registra en el hilo del vault; si falla, queda en el log.
            Some(note) => self.0.save(&note, text.to_owned(), kind).map(drop),
            None => Files.write(path, text, kind),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use metalmind_config::{HistoryRetention, Paths};
    use metalmind_vault::{History, HistoryOptions, VaultOptions, VaultPath, VersionKind};
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn una_nota_del_vault_deja_su_version_y_lo_de_fuera_no() {
        let (dir, data, outside) = (
            TempDir::new().unwrap(),
            TempDir::new().unwrap(),
            TempDir::new().unwrap(),
        );
        let options = VaultOptions {
            history: Some(HistoryOptions {
                paths: Paths::under(data.path()),
                retention: HistoryRetention::default(),
            }),
            ..VaultOptions::default()
        };
        let (vault, _updates) = Vault::open_with(dir.path(), options).unwrap();
        let vault = Rc::new(vault);
        let disk = VaultDisk::new(Rc::clone(&vault));
        let note = dir.path().join("a.md");
        disk.write(&note, "uno\n", SaveKind::Autosave).unwrap();
        disk.write(&outside.path().join("b.md"), "dos\n", SaveKind::Explicit)
            .unwrap();
        assert_eq!(fs::read_to_string(&note).unwrap(), "uno\n");
        assert_eq!(
            fs::read_to_string(outside.path().join("b.md")).unwrap(),
            "dos\n"
        );
        let root = vault.root().to_owned();
        // Cerrar el vault registra las versiones que esperan.
        drop((disk, vault));
        let paths = Paths::under(data.path());
        let history = History::new(
            paths.vault_data(&root).join("history"),
            HistoryRetention::default(),
        );
        let versions = history.versions(&VaultPath::new("a.md").unwrap()).unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].kind, VersionKind::Autosaved);
    }
}
