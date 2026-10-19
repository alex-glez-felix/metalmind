//! La retención en disco: rehacer el log de cada nota con las versiones que
//! le quedan y olvidar las notas borradas hace demasiado.

use std::collections::HashSet;
use std::time::SystemTime;

use jiff::tz::TimeZone;

use super::log::{DAMAGED, Entry, NoteLog};
use super::store::{IndexEntry, Lock};
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
        self.prune_while(now, || true)
    }

    /// Como [`History::prune`], pero deja de podar en cuanto `running` da
    /// `false`: lo que falte se poda la próxima vez.
    pub(crate) fn prune_while(
        &self,
        now: SystemTime,
        running: impl Fn() -> bool,
    ) -> Result<Pruned, HistoryError> {
        let now = to_millis(now);
        let deleted_for = millis(self.retention.deleted());
        let expired =
            |deleted: Option<i64>| deleted.is_some_and(|at| now.saturating_sub(at) > deleted_for);
        let mut pruned = Pruned::default();
        let index = self.store.read_index()?;
        // Sin historia en el índice ni logs sueltos no hay nada que podar.
        // Un índice con entradas se poda aunque no queden logs: el de una
        // nota pudo apartarse por dañado.
        if index.notes.is_empty() && index.set_aside.is_empty() && self.store.log_ids()?.is_empty()
        {
            return Ok(pruned);
        }
        let entries = index
            .notes
            .values()
            .chain(index.set_aside.iter().map(|(_, entry)| entry));
        for entry in entries.filter(|entry| !expired(entry.deleted)) {
            if !running() {
                return Ok(pruned);
            }
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
        let before = index.notes.len() + index.set_aside.len();
        let mut forgotten = Vec::new();
        let mut keep = |entry: &IndexEntry| {
            let keep = !expired(entry.deleted);
            if !keep {
                forgotten.push(entry.id);
            }
            keep
        };
        index.notes.retain(|_, entry| keep(entry));
        index.set_aside.retain(|(_, entry)| keep(entry));
        pruned.notes = before - index.notes.len() - index.set_aside.len();
        if pruned.notes > 0 {
            self.store.write_index(&lock, &index)?;
        }
        // Los logs que no están en el índice: los de recién olvidadas, y los
        // de un primer guardado que se cortó antes de escribir el índice.
        let known: HashSet<u64> = index
            .notes
            .values()
            .chain(index.set_aside.iter().map(|(_, entry)| entry))
            .map(|entry| entry.id)
            .collect();
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
        let kept = retention::keep(&saved_at, now, &self.retention, &TimeZone::system());
        let survivors: Vec<_> = log
            .entries()
            .iter()
            .zip(&kept)
            .filter_map(|(entry, keep)| keep.then_some(entry))
            .collect();
        let removed = saved_at.len() - survivors.len();
        // Las autoguardadas reemplazadas ya no son versiones, pero sus
        // registros ocupan lugar hasta rehacer el log.
        if removed == 0 && log.replaced() == 0 {
            return Ok(0);
        }
        self.rewrite_log(lock, id, &log, &survivors)?;
        Ok(removed)
    }

    /// Rehace el log `id` con solo las versiones `survivors` de `log`.
    pub(super) fn rewrite_log(
        &self,
        lock: &Lock,
        id: u64,
        log: &NoteLog,
        survivors: &[&Entry],
    ) -> Result<(), HistoryError> {
        let records: Vec<usize> = survivors.iter().map(|entry| entry.record).collect();
        let mut fresh = NoteLog::default();
        let mut bytes = Vec::new();
        let mut next = survivors.iter();
        let mut previous: Option<String> = None;
        log.for_each_content(&records, |text| {
            let entry = next.next().ok_or(DAMAGED)?;
            bytes.extend(fresh.push_version(entry, text, previous.as_deref())?);
            previous = Some(text.to_owned());
            Ok(())
        })
        .map_err(|corrupt| self.store.corrupt_log(id, corrupt))?;
        self.store.replace_log(lock, id, &bytes)
    }
}
