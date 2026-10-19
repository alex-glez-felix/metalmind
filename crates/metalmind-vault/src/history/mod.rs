//! Historial de versiones de cada nota (06 §3): se guarda una versión cada
//! vez que se guarda la nota, en el directorio de datos del vault, sin git;
//! las del autoguardado se agrupan (`kind.rs`).
//! Las versiones van como deltas por línea sobre la anterior, con snapshots
//! comprimidos cada tanto; el formato está en `log.rs` y la medición que lo
//! justifica en 03 §4.

mod bytes;
mod codec;
mod deleted;
mod delta;
mod diff;
mod follow;
mod frame;
mod kind;
mod log;
mod prune;
mod record;
mod retention;
mod store;

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use metalmind_config::HistoryRetention;

pub use deleted::{DeletedId, DeletedNote};
pub use diff::{ChunkKind, DiffChunk, RestoreEdit, RestorePlan, TextDiff};
pub use kind::{AUTOSAVE_GROUP, VersionKind};
pub use prune::Pruned;

use crate::file::FileError;
use crate::path::VaultPath;
use log::{Entry, NoteLog};
use store::{IndexEntry, Store};

/// El hash del contenido de una versión: dos versiones iguales comparten lo
/// que ocupan en disco.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContentHash([u8; 32]);

impl ContentHash {
    #[must_use]
    pub fn of(text: &str) -> Self {
        Self(*blake3::hash(text.as_bytes()).as_bytes())
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.0[..8] {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Una versión de una nota: cuándo se guardó y qué tenía. No cambia al
/// podar, que renumera las versiones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VersionId {
    at: i64,
    hash: ContentHash,
}

impl VersionId {
    fn of(entry: &Entry) -> Self {
        Self {
            at: entry.at,
            hash: entry.hash,
        }
    }
}

impl fmt::Display for VersionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{:?}", self.at, self.hash)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub id: VersionId,
    pub saved_at: SystemTime,
    pub hash: ContentHash,
    /// Bytes del contenido.
    pub len: usize,
    pub kind: VersionKind,
}

#[derive(Debug, thiserror::Error)]
pub enum HistoryError {
    #[error("historial: no se pudo {action} {}: {source}", path.display())]
    Io {
        action: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    #[error("historial dañado en {}: {reason}", path.display())]
    Corrupt { path: PathBuf, reason: &'static str },
    #[error("historial: {0}")]
    File(Box<FileError>),
    #[error("{path} no tiene una versión {id} en el historial")]
    UnknownVersion { path: VaultPath, id: VersionId },
    #[error("el historial ya no guarda esa nota borrada")]
    UnknownDeleted,
}

impl HistoryError {
    fn io(action: &'static str, path: &Path, source: io::Error) -> Self {
        Self::Io {
            action,
            path: path.to_owned(),
            source,
        }
    }

    fn corrupt(path: &Path, reason: &'static str) -> Self {
        Self::Corrupt {
            path: path.to_owned(),
            reason,
        }
    }
}

/// El historial de un vault, en su carpeta de datos. Cada llamada lee del
/// disco lo que necesita, con el lock de `store.rs`, así que varios procesos
/// (la app y la CLI) pueden usar el mismo a la vez.
pub struct History {
    store: Store,
    retention: HistoryRetention,
    latest: Mutex<Option<Latest>>,
}

/// El último contenido que se guardó: el delta del próximo guardado sale de
/// él sin reconstruirlo. Solo se usa si su hash es el de la última versión
/// en disco, así que otro proceso que haya escrito entremedio no lo engaña.
struct Latest {
    log: u64,
    hash: ContentHash,
    text: String,
}

impl History {
    /// El historial que vive en `dir`; no crea nada hasta el primer guardado.
    #[must_use]
    pub fn new(dir: impl Into<PathBuf>, retention: HistoryRetention) -> Self {
        Self {
            store: Store::new(dir.into()),
            retention,
            latest: Mutex::new(None),
        }
    }

    /// Las versiones de `path`, de la más vieja a la más nueva; ninguna si
    /// nunca se guardó. También las de una nota borrada.
    ///
    /// # Errors
    /// Si no se puede leer el historial.
    pub fn versions(&self, path: &VaultPath) -> Result<Vec<Version>, HistoryError> {
        let _lock = self.store.read_lock()?;
        let Some(entry) = self.store.read_index()?.notes.get(path).copied() else {
            return Ok(Vec::new());
        };
        let log = self.store.read_log(entry.id)?;
        Ok(log.entries().iter().map(version).collect())
    }

    /// El contenido exacto de una versión.
    ///
    /// # Errors
    /// Si la versión no existe o no se puede leer.
    pub fn content(&self, path: &VaultPath, id: VersionId) -> Result<String, HistoryError> {
        let _lock = self.store.read_lock()?;
        let (entry, log) = self.find(path, id)?;
        self.version_content(entry.id, &log, id)?
            .ok_or_else(|| unknown(path, id))
    }

    /// El contenido de la versión `id` del log `log_id`, si la tiene.
    fn version_content(
        &self,
        log_id: u64,
        log: &NoteLog,
        id: VersionId,
    ) -> Result<Option<String>, HistoryError> {
        let Some(version) = log.entries().iter().find(|e| VersionId::of(e) == id) else {
            return Ok(None);
        };
        log.version_text(version)
            .map(Some)
            .map_err(|corrupt| self.store.corrupt_log(log_id, corrupt))
    }

    /// El diff por líneas de la versión `from` a la `to`.
    ///
    /// # Errors
    /// Si alguna de las dos no existe o no se puede leer.
    pub fn diff(
        &self,
        path: &VaultPath,
        from: VersionId,
        to: VersionId,
    ) -> Result<TextDiff, HistoryError> {
        Ok(TextDiff::new(
            self.content(path, from)?,
            self.content(path, to)?,
        ))
    }

    /// Lo que hay que cambiar de `current` para volver a la versión `id`. La
    /// UI lo aplica al buffer como una edición que se deshace; al guardar,
    /// esa vuelta atrás es una versión más.
    ///
    /// # Errors
    /// Si la versión no existe o no se puede leer.
    pub fn restore_plan(
        &self,
        path: &VaultPath,
        id: VersionId,
        current: &str,
    ) -> Result<RestorePlan, HistoryError> {
        Ok(RestorePlan::new(current, &self.content(path, id)?))
    }

    fn take_latest(&self, log_id: u64, log: &NoteLog) -> Option<Latest> {
        let latest = self
            .latest
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()?;
        let last = log.entries().last()?;
        (latest.log == log_id && latest.hash == last.hash).then_some(latest)
    }

    fn find(&self, path: &VaultPath, id: VersionId) -> Result<(IndexEntry, NoteLog), HistoryError> {
        let entry = self
            .store
            .read_index()?
            .notes
            .get(path)
            .copied()
            .ok_or_else(|| unknown(path, id))?;
        Ok((entry, self.store.read_log(entry.id)?))
    }
}

fn version(entry: &Entry) -> Version {
    Version {
        id: VersionId::of(entry),
        saved_at: from_millis(entry.at),
        hash: entry.hash,
        len: entry.len,
        kind: entry.kind,
    }
}

fn unknown(path: &VaultPath, id: VersionId) -> HistoryError {
    HistoryError::UnknownVersion {
        path: path.clone(),
        id,
    }
}

fn millis(duration: Duration) -> i64 {
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

fn to_millis(at: SystemTime) -> i64 {
    match at.duration_since(UNIX_EPOCH) {
        Ok(after) => millis(after),
        Err(before) => -millis(before.duration()),
    }
}

fn from_millis(millis: i64) -> SystemTime {
    let duration = Duration::from_millis(millis.unsigned_abs());
    if millis >= 0 {
        UNIX_EPOCH + duration
    } else {
        UNIX_EPOCH - duration
    }
}
