//! La retención en disco: rehacer el log de cada nota con las versiones que
//! le quedan y olvidar las notas borradas hace demasiado.

use std::collections::HashSet;
use std::time::SystemTime;

use super::log::{DAMAGED, NoteLog};
use super::store::Lock;
use super::{History, HistoryError, millis, retention, to_millis};

/// Lo que se llevó una poda.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Pruned {
    pub versions: usize,
    /// Historias enteras de notas borradas hace más de lo que dice la retención.
    pub notes: usize,
}

impl History {
    /// Aplica la retención: borra las versiones que ya no le tocan y la
    /// historia de las notas borradas hace demasiado. Toma el lock nota por
    /// nota, así que un guardado espera a lo sumo lo que tarda en podarse una.
    ///
    /// # Errors
    /// Si no se puede leer o escribir el historial.
    pub fn prune(&self, now: SystemTime) -> Result<Pruned, HistoryError> {
        let now = to_millis(now);
        let deleted_for = millis(self.retention.deleted());
        let expired =
            |deleted: Option<i64>| deleted.is_some_and(|at| now.saturating_sub(at) > deleted_for);
        let mut pruned = Pruned::default();
        if self.store.log_ids()?.is_empty() {
            return Ok(pruned);
        }
        let index = self.store.read_index()?;
        for entry in index.notes.values().filter(|entry| !expired(entry.deleted)) {
            let lock = self.store.lock()?;
            match self.prune_log(&lock, entry.id, now) {
                Ok(versions) => pruned.versions += versions,
                // Un log dañado se queda como está; las demás notas se podan igual.
                Err(error @ HistoryError::Corrupt { .. }) => log::warn!("{error}"),
                Err(error) => return Err(error),
            }
        }

        let lock = self.store.lock()?;
        let mut index = self.store.read_index()?;
        let before = index.notes.len();
        let mut forgotten = Vec::new();
        index.notes.retain(|_, entry| {
            let keep = !expired(entry.deleted);
            if !keep {
                forgotten.push(entry.id);
            }
            keep
        });
        pruned.notes = before - index.notes.len();
        if pruned.notes > 0 {
            self.store.write_index(&lock, &index)?;
        }
        // Los logs que no están en el índice: los de recién olvidadas, y los
        // de un primer guardado que se cortó antes de escribir el índice.
        let known: HashSet<u64> = index.notes.values().map(|entry| entry.id).collect();
        for id in self.store.log_ids()?.into_iter().chain(forgotten) {
            if !known.contains(&id) {
                self.store.remove_log(&lock, id)?;
            }
        }
        Ok(pruned)
    }

    fn prune_log(&self, lock: &Lock, id: u64, now: i64) -> Result<usize, HistoryError> {
        let log = self.store.read_log(id)?;
        let saved_at: Vec<i64> = log.entries().iter().map(|entry| entry.at).collect();
        let kept = retention::keep(&saved_at, now, &self.retention);
        let survivors: Vec<_> = log
            .entries()
            .iter()
            .zip(&kept)
            .filter_map(|(entry, keep)| keep.then_some(entry))
            .collect();
        let removed = saved_at.len() - survivors.len();
        if removed == 0 {
            return Ok(0);
        }
        let records: Vec<usize> = survivors.iter().map(|entry| entry.record).collect();
        let mut fresh = NoteLog::default();
        let mut bytes = Vec::new();
        let mut next = survivors.iter();
        let mut previous: Option<String> = None;
        log.for_each_content(&records, |text| {
            let entry = next.next().ok_or(DAMAGED)?;
            bytes.extend(fresh.push_version(entry.seq, entry.at, text, previous.as_deref())?);
            previous = Some(text.to_owned());
            Ok(())
        })
        .map_err(|corrupt| self.store.corrupt_log(id, corrupt))?;
        self.store.replace_log(lock, id, &bytes)?;
        Ok(removed)
    }
}
