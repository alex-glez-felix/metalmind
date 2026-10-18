//! Registrar versiones: una al guardar, o las de un lote de cambios desde
//! fuera con una sola lectura y escritura del índice.

use std::sync::PoisonError;
use std::time::SystemTime;

use super::log::NoteLog;
use super::store::{Index, IndexEntry, Lock};
use super::{History, HistoryError, Latest, VersionId, to_millis};
use crate::path::VaultPath;

impl History {
    /// Registra `text` como versión de `path`. Devuelve `None` si es igual a
    /// la última y no hacía falta guardar nada. Si la nota estaba borrada, su
    /// historia sigue.
    ///
    /// # Errors
    /// Si no se puede leer o escribir el historial.
    pub fn record(
        &self,
        path: &VaultPath,
        text: &str,
        at: SystemTime,
    ) -> Result<Option<VersionId>, HistoryError> {
        let lock = self.store.lock()?;
        let mut index = self.store.read_index()?;
        let (recorded, index_changed) = self.record_in(&lock, &mut index, path, text, at)?;
        if index_changed {
            self.store.write_index(&lock, &index)?;
        }
        Ok(recorded)
    }

    /// Registra una versión de cada nota. Un log dañado no impide registrar
    /// las demás: queda en el log de la app.
    ///
    /// # Errors
    /// Si no se puede leer o escribir el índice, o escribir algún log.
    pub(crate) fn record_all(
        &self,
        notes: impl IntoIterator<Item = (VaultPath, String)>,
        at: SystemTime,
    ) -> Result<(), HistoryError> {
        let mut notes = notes.into_iter().peekable();
        if notes.peek().is_none() {
            return Ok(());
        }
        let lock = self.store.lock()?;
        let mut index = self.store.read_index()?;
        let mut index_changed = false;
        for (path, text) in notes {
            match self.record_in(&lock, &mut index, &path, &text, at) {
                Ok((_, changed)) => index_changed |= changed,
                Err(error @ HistoryError::Corrupt { .. }) => log::warn!("{error}"),
                Err(error) => return Err(error),
            }
        }
        if index_changed {
            self.store.write_index(&lock, &index)?;
        }
        Ok(())
    }

    /// Agrega la versión al log de `path` y deja `index` como debe quedar;
    /// dice si el índice cambió y hay que escribirlo.
    fn record_in(
        &self,
        lock: &Lock,
        index: &mut Index,
        path: &VaultPath,
        text: &str,
        at: SystemTime,
    ) -> Result<(Option<VersionId>, bool), HistoryError> {
        let (entry, mut log, index_changed) = match index.notes.get(path) {
            Some(entry) => (
                *entry,
                self.readable_log(lock, entry.id)?,
                entry.deleted.is_some(),
            ),
            // Un id nuevo empieza un log vacío aunque en disco haya uno que un
            // guardado cortado dejó fuera del índice: `append_log` lo pisa.
            None => (index.allocate(path.clone()), NoteLog::default(), true),
        };
        let valid_len = log.valid_len();
        let cached = self.take_latest(entry.id, &log);
        let appended = log
            .append(
                text,
                to_millis(at),
                cached.as_ref().map(|latest| latest.text.as_str()),
            )
            .map_err(|corrupt| self.store.corrupt_log(entry.id, corrupt))?;
        if let Some((_, frames)) = &appended {
            self.store.append_log(lock, entry.id, valid_len, frames)?;
        }
        if let Some(last) = log.entries().last() {
            *self.latest.lock().unwrap_or_else(PoisonError::into_inner) = Some(Latest {
                log: entry.id,
                hash: last.hash,
                text: text.to_owned(),
            });
        }
        if index_changed {
            index.notes.insert(
                path.clone(),
                IndexEntry {
                    deleted: None,
                    ..entry
                },
            );
        }
        let recorded = appended.and(log.entries().last()).map(VersionId::of);
        Ok((recorded, index_changed))
    }

    /// El log `id`, o uno vacío si el de disco está dañado: se aparta y la
    /// nota sigue guardando versiones en vez de fallar para siempre.
    fn readable_log(&self, lock: &Lock, id: u64) -> Result<NoteLog, HistoryError> {
        match self.store.read_log(id) {
            Err(error @ HistoryError::Corrupt { .. }) => {
                log::warn!("{error}; se aparta y empieza uno nuevo");
                self.store.set_aside(lock, id)?;
                Ok(NoteLog::default())
            }
            other => other,
        }
    }
}
