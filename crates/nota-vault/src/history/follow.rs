//! El historial sigue lo que le pasa a las notas: renombres y borrados que
//! ve el watcher, y lo que pasó con el vault cerrado (06 §1).

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::time::SystemTime;

use super::log::{DAMAGED, NoteLog};
use super::store::{Index, IndexEntry, Lock};
use super::{ContentHash, History, HistoryError, to_millis};
use crate::file;
use crate::index::EntryKind;
use crate::path::VaultPath;
use crate::refresh::VaultChange;

impl History {
    /// `from` pasó a llamarse `to`. Si `from` es una carpeta, se mueve la
    /// historia de cada nota que tenía dentro. Si `to` ya tenía historia (una
    /// nota borrada ahí, o pisada desde fuera), las dos se juntan por fecha y
    /// se renumeran.
    ///
    /// # Errors
    /// Si no se puede leer o escribir el historial.
    pub fn rename(&self, from: &VaultPath, to: &VaultPath) -> Result<(), HistoryError> {
        self.edit_index(|lock, index, orphans| self.rename_in(lock, index, from, to, orphans))
    }

    /// `path` se borró en `at`. Su historia se guarda lo que diga la
    /// retención, por si se quiere recuperar.
    ///
    /// # Errors
    /// Si no se puede leer o escribir el historial.
    pub fn mark_deleted(&self, path: &VaultPath, at: SystemTime) -> Result<(), HistoryError> {
        self.edit_index(|_, index, _| Ok(mark_deleted_in(index, path, at)))
    }

    /// Los renombres y borrados de un lote del watcher, en orden, con una
    /// sola lectura y escritura del índice. Una nota borrada cuyo contenido
    /// aparece en una creada del mismo lote se renombró: así llegan dos
    /// renombres seguidos, o el borrar y crear de `git checkout` (06 §1).
    pub(crate) fn follow(
        &self,
        root: &Path,
        changes: &[VaultChange],
        now: SystemTime,
    ) -> Result<(), HistoryError> {
        // Qué historias toca el lote se decide con el índice leído bajo el
        // lock: otro proceso puede haber registrado una nota recién.
        let follows = |change: &VaultChange| {
            matches!(
                change,
                VaultChange::Renamed { .. }
                    | VaultChange::Removed {
                        kind: EntryKind::Note,
                        ..
                    }
            )
        };
        if !changes.iter().any(follows) {
            return Ok(());
        }
        let created: HashSet<&VaultPath> = changes
            .iter()
            .filter_map(|change| match change {
                VaultChange::Created {
                    path,
                    kind: EntryKind::Note,
                } => Some(path),
                _ => None,
            })
            .collect();
        self.edit_index(|lock, index, orphans| {
            let removed: Vec<VaultPath> = changes
                .iter()
                .filter_map(|change| match change {
                    VaultChange::Removed {
                        path,
                        kind: EntryKind::Note,
                    } if index.notes.get(path).is_some_and(|e| e.deleted.is_none()) => {
                        Some(path.clone())
                    }
                    _ => None,
                })
                .collect();
            let mut moved = self.adopt(root, index, &removed, &created)?;
            let mut edited = false;
            for change in changes {
                edited |= match change {
                    VaultChange::Renamed { from, to, .. } => {
                        self.rename_in(lock, index, from, to, orphans)?
                    }
                    VaultChange::Removed {
                        path,
                        kind: EntryKind::Note,
                    } => match moved.remove(path) {
                        Some(to) => self.rename_in(lock, index, path, to, orphans)?,
                        None => mark_deleted_in(index, path, now),
                    },
                    _ => false,
                };
            }
            Ok(edited)
        })
    }

    /// Alinea el historial con el vault recién abierto. Una historia viva
    /// cuya nota ya no está pasa a la nota con el mismo contenido que su
    /// última versión (se renombró con nota cerrada), y se junta con la que
    /// hubiera ahí de una nota borrada; si no hay ninguna, la nota se borró.
    /// Una borrada cuya ruta volvió, revive.
    pub(crate) fn reconcile(
        &self,
        root: &Path,
        notes: &HashSet<&VaultPath>,
        now: SystemTime,
    ) -> Result<(), HistoryError> {
        // Sin lock alcanza: lo que alinear viene de un índice que ya existía,
        // y lo que otro proceso registre mientras tanto es de una nota viva.
        if self.store.read_index()?.notes.is_empty() {
            return Ok(());
        }
        self.edit_index(|lock, index, orphans| {
            let before = index.clone();
            let lost: Vec<VaultPath> = index
                .notes
                .iter()
                .filter(|(path, entry)| entry.deleted.is_none() && !notes.contains(path))
                .map(|(path, _)| path.clone())
                .collect();
            let mut adopted = self.adopt(root, index, &lost, notes)?;
            for path in lost {
                match adopted.remove(&path) {
                    Some(new) => {
                        self.rename_in(lock, index, &path, new, orphans)?;
                    }
                    None => {
                        mark_deleted_in(index, &path, now);
                    }
                }
            }
            for (_, entry) in index
                .notes
                .iter_mut()
                .filter(|(path, _)| notes.contains(path))
            {
                entry.deleted = None;
            }
            Ok(*index != before)
        })
    }

    /// Toma el lock, lee el índice, le aplica `edit` y lo escribe si cambió.
    /// Los logs que `edit` deja sin usar se borran solo después de escribir
    /// el índice, para que el de disco nunca nombre un log que ya no está; si
    /// algo falla antes, quedan para la poda.
    fn edit_index(
        &self,
        edit: impl FnOnce(&Lock, &mut Index, &mut Vec<u64>) -> Result<bool, HistoryError>,
    ) -> Result<(), HistoryError> {
        let lock = self.store.lock()?;
        let mut index = self.store.read_index()?;
        let mut orphans = Vec::new();
        if edit(&lock, &mut index, &mut orphans)? {
            self.store.write_index(&lock, &index)?;
            for id in orphans {
                self.store.remove_log(&lock, id)?;
            }
        }
        Ok(())
    }

    fn rename_in(
        &self,
        lock: &Lock,
        index: &mut Index,
        from: &VaultPath,
        to: &VaultPath,
        orphans: &mut Vec<u64>,
    ) -> Result<bool, HistoryError> {
        let moved: Vec<(VaultPath, VaultPath)> = index_under(index, from)
            .filter_map(|path| Some((path.clone(), path.rebase(from, to)?)))
            .collect();
        for (old, new) in &moved {
            let Some(mut entry) = index.notes.remove(old) else {
                continue;
            };
            if let Some(&existing) = index.notes.get(new) {
                match self.merge(lock, entry, existing) {
                    Ok(()) => orphans.push(existing.id),
                    // Sin juntar: queda la historia sana y la dañada se aparta.
                    Err(HistoryError::Corrupt { path, reason }) => {
                        log::warn!("historial dañado en {}: {reason}", path.display());
                        let damaged = if self.store.is_log(&path, entry.id) {
                            std::mem::replace(&mut entry, existing)
                        } else {
                            existing
                        };
                        self.store.set_aside(lock, damaged.id)?;
                    }
                    Err(error) => return Err(error),
                }
            }
            // La nota misma se movió, así que está viva en `to`. Las borradas
            // dentro de una carpeta movida siguen borradas.
            if old == from {
                entry.deleted = None;
            }
            index.notes.insert(new.clone(), entry);
        }
        Ok(!moved.is_empty())
    }

    /// Para cada historia de `lost`, la nota cuyo contenido es igual a su
    /// última versión y que no tiene historia viva. Solo cuando el
    /// emparejamiento no tiene dudas: una nota vacía, o un contenido que
    /// tiene más de una historia o más de una nota, se queda sin adoptar.
    /// Mira el tamaño antes de leer y hashear.
    fn adopt<'a>(
        &self,
        root: &Path,
        index: &Index,
        lost: &[VaultPath],
        notes: &HashSet<&'a VaultPath>,
    ) -> Result<HashMap<VaultPath, &'a VaultPath>, HistoryError> {
        let mut wanted: HashMap<(usize, ContentHash), Vec<VaultPath>> = HashMap::new();
        for path in lost {
            let Some(entry) = index.notes.get(path) else {
                continue;
            };
            let latest = match self.store.read_log(entry.id) {
                Ok(log) => log.entries().last().map(|last| (last.len, last.hash)),
                // Un log dañado no impide alinear las demás notas.
                Err(error @ HistoryError::Corrupt { .. }) => {
                    log::warn!("{error}");
                    None
                }
                Err(error) => return Err(error),
            };
            if let Some(key @ (len, _)) = latest
                && len > 0
            {
                wanted.entry(key).or_default().push(path.clone());
            }
        }
        let sizes: HashSet<usize> = wanted.keys().map(|(len, _)| *len).collect();
        if sizes.is_empty() {
            return Ok(HashMap::new());
        }
        let mut found: HashMap<(usize, ContentHash), Vec<&'a VaultPath>> = HashMap::new();
        let free = |note: &VaultPath| {
            index
                .notes
                .get(note)
                .is_none_or(|entry| entry.deleted.is_some())
        };
        for &note in notes.iter().filter(|note| free(note)) {
            let full = note.to_fs(root);
            let same_size = fs::metadata(&full)
                .ok()
                .and_then(|metadata| usize::try_from(metadata.len()).ok())
                .is_some_and(|len| sizes.contains(&len));
            if !same_size {
                continue;
            }
            let Ok(Some(text)) = file::read(&full) else {
                continue;
            };
            let key = (text.len(), ContentHash::of(&text));
            if wanted.contains_key(&key) {
                found.entry(key).or_default().push(note);
            }
        }
        // Con dos historias del mismo contenido, nada dice cuál fue a dónde.
        Ok(wanted
            .into_iter()
            .filter_map(|(key, histories)| {
                match (histories.as_slice(), found.remove(&key)?.as_slice()) {
                    ([history], [target]) => Some((history.clone(), *target)),
                    _ => None,
                }
            })
            .collect())
    }

    /// Junta la historia de `source` con la de `target`, por fecha, en el
    /// log de `source`. Las versiones se renumeran; el log de `target` queda
    /// sin usar y lo borra quien escriba el índice.
    fn merge(
        &self,
        lock: &Lock,
        source: IndexEntry,
        target: IndexEntry,
    ) -> Result<(), HistoryError> {
        let logs = [
            self.store.read_log(target.id)?,
            self.store.read_log(source.id)?,
        ];
        let ids = [target.id, source.id];
        let records = logs
            .each_ref()
            .map(|log| log.entries().iter().map(|e| e.record).collect::<Vec<_>>());
        let mut contents = Vec::with_capacity(2);
        for (log, (records, id)) in logs.iter().zip(records.iter().zip(ids)) {
            contents.push(
                log.contents(records)
                    .map_err(|corrupt| self.store.corrupt_log(id, corrupt))?,
            );
        }
        let mut saved = logs
            .each_ref()
            .map(|log| log.entries().iter().map(|e| e.at).peekable());
        let mut fresh = NoteLog::default();
        let mut bytes = Vec::new();
        let mut previous: Option<String> = None;
        for seq in 0.. {
            // Por fecha; en un empate, primero la de `target`, que estaba antes.
            let older = saved[0].peek().copied();
            let newer = saved[1].peek().copied();
            let side = match (older, newer) {
                (None, None) => break,
                (Some(older), Some(newer)) if older > newer => 1,
                (Some(_), _) => 0,
                (None, Some(_)) => 1,
            };
            let at = saved[side].next().ok_or(DAMAGED);
            let text = at.and_then(|at| Ok((at, contents[side].next_text()?.ok_or(DAMAGED)?)));
            let (at, text) = text.map_err(|corrupt| self.store.corrupt_log(ids[side], corrupt))?;
            let frames = fresh
                .push_version(seq, at, &text, previous.as_deref())
                .map_err(|corrupt| self.store.corrupt_log(source.id, corrupt))?;
            bytes.extend(frames);
            previous = Some(text);
        }
        self.store.replace_log(lock, source.id, &bytes)
    }
}

/// Lo que el índice tiene en `path` o dentro de ella.
fn index_under<'a>(index: &'a Index, path: &'a VaultPath) -> impl Iterator<Item = &'a VaultPath> {
    // Las rutas que empiezan con el texto de `path` van juntas en el orden del mapa.
    index
        .notes
        .range(path.clone()..)
        .map(|(candidate, _)| candidate)
        .take_while(move |candidate| candidate.as_str().starts_with(path.as_str()))
        .filter(move |candidate| candidate.starts_with(path))
}

fn mark_deleted_in(index: &mut Index, path: &VaultPath, at: SystemTime) -> bool {
    match index.notes.get_mut(path) {
        Some(entry) if entry.deleted.is_none() => {
            entry.deleted = Some(to_millis(at));
            true
        }
        _ => false,
    }
}
