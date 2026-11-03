//! La caché en disco de lo escaneado de cada nota (03 §4): al abrir, una nota
//! con el mismo tamaño y mtime no se lee, y una con el mismo hash no se
//! vuelve a escanear. Los `.md` son la verdad: lo que falte, no cuadre o sea
//! de otra versión se saca de ellos, y borrar la caché solo cuesta releer.

mod format;
#[cfg(test)]
mod tests;

use std::borrow::Cow;
use std::fs;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::history::ContentHash;
use crate::links::LinkIndex;
use crate::note_scan::NoteScan;
use crate::parallel::in_parallel;
use crate::path::VaultPath;
use crate::properties::PropertyIndex;
use crate::tags::{ReadNote, Tags};
use format::NoteRecord;

/// Un mtime más reciente que esto no prueba nada: otra escritura en el mismo
/// instante, con la granularidad del sistema de archivos, no lo cambiaría.
/// Dos segundos cubren al peor común, FAT.
const SETTLE_SECONDS: i64 = 2;

/// Lo que se puede saber de una nota sin abrirla.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Stamp {
    size: u64,
    modified_seconds: i64,
    modified_nanos: i64,
}

impl Stamp {
    /// El de `file`, siguiendo enlaces simbólicos como al leerlo.
    pub(crate) fn of_file(file: &Path) -> Option<Self> {
        let metadata = fs::metadata(file).ok()?;
        Some(Self {
            size: metadata.size(),
            modified_seconds: metadata.mtime(),
            modified_nanos: metadata.mtime_nsec(),
        })
    }

    /// Si la nota podría cambiar sin que cambie este sello.
    fn is_settled(self, now_seconds: i64) -> bool {
        self.modified_seconds <= now_seconds - SETTLE_SECONDS
    }
}

/// Lo que dice la caché de una nota: el sello solo si es de fiar.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CachedNote {
    stamp: Option<Stamp>,
    hash: ContentHash,
    scan: NoteScan,
}

/// Las notas de un vault al abrirlo.
#[derive(Debug)]
pub(crate) struct OpenedNotes {
    /// Todas; una que no se pudo leer, con un escaneo vacío.
    pub scans: Vec<(VaultPath, NoteScan)>,
    /// `None` si la caché ya dice lo mismo, o si no hay caché.
    pub refresh: Option<CacheRefresh>,
}

/// Qué escribir en la caché; lo escaneado sale de los índices, que ya lo
/// tienen, para no copiarlo al abrir.
#[derive(Debug)]
pub(crate) struct CacheRefresh {
    file: PathBuf,
    notes: Vec<NoteVersion>,
}

#[derive(Debug)]
struct NoteVersion {
    path: VaultPath,
    stamp: Option<Stamp>,
    hash: ContentHash,
}

struct OpenedNote {
    path: VaultPath,
    scan: NoteScan,
    /// `None` si no se pudo leer: no entra en la caché y se reintenta.
    version: Option<NoteVersion>,
    from_cache: bool,
}

/// El escaneo de cada una de `notes`, en varios hilos, con la caché de `file`.
/// El sello va antes que la lectura: si la nota cambia entre los dos, el
/// sello guardado es viejo y la próxima apertura la relee.
pub(crate) fn open_notes(
    file: Option<&Path>,
    notes: Vec<VaultPath>,
    stamp_of: &(impl Fn(&VaultPath) -> Option<Stamp> + Sync),
    read: &impl ReadNote,
) -> OpenedNotes {
    let mut cached = file.map(format::load).unwrap_or_default();
    let notes: Vec<_> = notes
        .into_iter()
        .map(|path| {
            let note = cached.remove(path.as_str());
            (path, note)
        })
        .collect();
    let some_gone = !cached.is_empty();
    let now = now_seconds();
    let opened = in_parallel(notes, |(path, note)| {
        open_note(path, note, now, stamp_of, read)
    });
    let current = !some_gone && opened.iter().all(|note| note.from_cache);
    let mut scans = Vec::with_capacity(opened.len());
    let mut versions = Vec::with_capacity(opened.len());
    for note in opened {
        scans.push((note.path, note.scan));
        versions.extend(note.version);
    }
    versions.sort_unstable_by(|a, b| a.path.cmp(&b.path));
    let refresh = file.filter(|_| !current).map(|file| CacheRefresh {
        file: file.to_owned(),
        notes: versions,
    });
    OpenedNotes { scans, refresh }
}

fn open_note(
    path: VaultPath,
    cached: Option<CachedNote>,
    now: i64,
    stamp_of: &impl Fn(&VaultPath) -> Option<Stamp>,
    read: &impl ReadNote,
) -> OpenedNote {
    let stamp = stamp_of(&path);
    match cached {
        Some(note) if stamp.is_some() && note.stamp == stamp => OpenedNote {
            version: Some(NoteVersion {
                path: path.clone(),
                stamp,
                hash: note.hash,
            }),
            path,
            scan: note.scan,
            from_cache: true,
        },
        cached => read_note(path, cached, stamp.filter(|s| s.is_settled(now)), read),
    }
}

fn read_note(
    path: VaultPath,
    cached: Option<CachedNote>,
    stamp: Option<Stamp>,
    read: &impl ReadNote,
) -> OpenedNote {
    let Some(text) = read(&path) else {
        return OpenedNote {
            path,
            scan: NoteScan::default(),
            version: None,
            from_cache: false,
        };
    };
    let hash = ContentHash::of(&text);
    let scan = match cached {
        // Un `touch`, o un mtime que todavía no era de fiar.
        Some(note) if note.hash == hash => note.scan,
        _ => NoteScan::of(&text),
    };
    OpenedNote {
        version: Some(NoteVersion {
            path: path.clone(),
            stamp,
            hash,
        }),
        path,
        scan,
        from_cache: false,
    }
}

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|now| i64::try_from(now.as_secs()).ok())
        .unwrap_or_default()
}

impl CacheRefresh {
    /// Reescribe la caché con los índices recién armados. Si falla, la
    /// próxima apertura lee lo que haga falta: no es para molestar a nadie.
    pub(crate) fn write(&self, links: &LinkIndex, tags: &Tags, properties: &PropertyIndex) {
        let records = self.notes.iter().filter_map(|note| {
            Some(NoteRecord {
                path: Cow::Borrowed(note.path.as_str()),
                stamp: note.stamp,
                hash: note.hash,
                links: Cow::Borrowed(links.outgoing(&note.path)),
                tags: Cow::Borrowed(tags.index().note(&note.path)?),
                properties: Cow::Borrowed(properties.of(&note.path)),
            })
        });
        let records: Vec<NoteRecord<'_>> = records.collect();
        if let Err(error) = format::save(&self.file, &records) {
            log::warn!("{error}");
        }
    }
}
