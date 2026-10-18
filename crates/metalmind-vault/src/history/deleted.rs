//! Las notas borradas cuya historia todavía se guarda: las borradas en su
//! ruta y las que quedaron aparte porque un renombre llegó encima.

use std::time::SystemTime;

use super::{History, HistoryError, Version, VersionId, from_millis, version};
use crate::path::VaultPath;

/// Una nota borrada cuya historia todavía se guarda.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletedNote {
    pub path: VaultPath,
    pub deleted_at: SystemTime,
    /// Para leer sus versiones aunque en su ruta haya otra historia.
    pub id: DeletedId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeletedId(u64);

impl History {
    /// Las notas borradas cuya historia todavía se guarda, por ruta y fecha.
    ///
    /// # Errors
    /// Si no se puede leer el historial.
    pub fn deleted_notes(&self) -> Result<Vec<DeletedNote>, HistoryError> {
        let _lock = self.store.read_lock()?;
        let index = self.store.read_index()?;
        let mut deleted: Vec<DeletedNote> = index
            .notes
            .into_iter()
            .chain(index.set_aside)
            .filter_map(|(path, entry)| {
                Some(DeletedNote {
                    path,
                    deleted_at: from_millis(entry.deleted?),
                    id: DeletedId(entry.id),
                })
            })
            .collect();
        deleted.sort_by(|a, b| (&a.path, a.deleted_at).cmp(&(&b.path, b.deleted_at)));
        Ok(deleted)
    }

    /// Las versiones de una nota borrada, de la más vieja a la más nueva.
    ///
    /// # Errors
    /// Si el historial ya no la guarda o no se puede leer.
    pub fn deleted_versions(&self, id: DeletedId) -> Result<Vec<Version>, HistoryError> {
        let _lock = self.store.read_lock()?;
        self.check_deleted(id)?;
        let log = self.store.read_log(id.0)?;
        Ok(log.entries().iter().map(version).collect())
    }

    /// El contenido exacto de una versión de una nota borrada.
    ///
    /// # Errors
    /// Si el historial ya no la guarda, no tiene esa versión o no se puede leer.
    pub fn deleted_content(
        &self,
        id: DeletedId,
        version: VersionId,
    ) -> Result<String, HistoryError> {
        let _lock = self.store.read_lock()?;
        self.check_deleted(id)?;
        let log = self.store.read_log(id.0)?;
        self.version_content(id.0, &log, version)?
            .ok_or(HistoryError::UnknownDeleted)
    }

    fn check_deleted(&self, id: DeletedId) -> Result<(), HistoryError> {
        let index = self.store.read_index()?;
        let mut entries = index
            .notes
            .values()
            .chain(index.set_aside.iter().map(|(_, entry)| entry));
        if entries.any(|entry| entry.id == id.0 && entry.deleted.is_some()) {
            Ok(())
        } else {
            Err(HistoryError::UnknownDeleted)
        }
    }
}
