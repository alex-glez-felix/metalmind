use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arc_swap::ArcSwap;

use crate::index::VaultIndex;
use crate::scan::scan;

#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("no se pudo abrir el vault {}: {source}", root.display())]
    Open { root: PathBuf, source: io::Error },
    #[error("{} no es una carpeta", .0.display())]
    NotADirectory(PathBuf),
}

/// Un vault abierto: la carpeta raíz y su índice.
pub struct Vault {
    root: PathBuf,
    index: Arc<ArcSwap<VaultIndex>>,
}

impl Vault {
    /// Abre la carpeta `root` y la indexa.
    ///
    /// # Errors
    /// Si `root` no existe o no es una carpeta.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, VaultError> {
        let root = canonical_dir(root.as_ref())?;
        let index = scan(&root);
        Ok(Self {
            root,
            index: Arc::new(ArcSwap::from_pointee(index)),
        })
    }

    /// La raíz, sin enlaces simbólicos.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// El índice de este momento. No bloquea ni se modifica después.
    #[must_use]
    pub fn snapshot(&self) -> Arc<VaultIndex> {
        self.index.load_full()
    }
}

fn canonical_dir(root: &Path) -> Result<PathBuf, VaultError> {
    let canonical = root.canonicalize().map_err(|source| VaultError::Open {
        root: root.to_owned(),
        source,
    })?;
    if canonical.is_dir() {
        Ok(canonical)
    } else {
        Err(VaultError::NotADirectory(root.to_owned()))
    }
}
