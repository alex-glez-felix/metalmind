//! Dónde vive el historial de un vault: `index` (qué nota es cada log, y si
//! se borró) y `notes/<id>.log`. Renombrar una nota solo cambia el índice.
//! Las escrituras toman el lock de `lock` en exclusiva (también entre
//! procesos: la CLI y la app) y las lecturas compartido, para no ver un
//! índice que nombra un log que la poda acaba de borrar.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::PathBuf;

use rustix::fs::{FlockOperation, flock};

use super::HistoryError;
use super::bytes::{Reader, put_bytes, put_var};
use super::log::{Corrupt, NoteLog};
use crate::file::write_atomic;
use crate::path::VaultPath;

const INDEX_MAGIC: &[u8; 8] = b"notaidx1";
const NO_DELETION: i64 = i64::MIN;
const NOTE: u8 = 0;
const SET_ASIDE: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct IndexEntry {
    pub id: u64,
    /// Cuándo se borró la nota, en milisegundos desde 1970.
    pub deleted: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Index {
    pub notes: BTreeMap<VaultPath, IndexEntry>,
    /// Historias de notas borradas cuya ruta ya tiene otra historia: un
    /// renombre llegó encima. Siempre con fecha de borrado.
    pub set_aside: Vec<(VaultPath, IndexEntry)>,
    pub next_id: u64,
}

impl Index {
    pub(super) fn allocate(&mut self, path: VaultPath) -> IndexEntry {
        let entry = IndexEntry {
            id: self.next_id,
            deleted: None,
        };
        self.next_id += 1;
        self.notes.insert(path, entry);
        entry
    }
}

pub(super) struct Store {
    dir: PathBuf,
}

/// Mientras exista, nadie más escribe en el historial.
pub(super) struct Lock {
    _file: File,
}

/// Mientras exista, nadie escribe en el historial, pero otros pueden leer.
pub(super) struct ReadLock {
    _file: Option<File>,
}

impl Store {
    pub(super) fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub(super) fn lock(&self) -> Result<Lock, HistoryError> {
        let path = self.dir.join("lock");
        let failed = |source| HistoryError::io("bloquear", &path, source);
        fs::create_dir_all(self.notes_dir()).map_err(failed)?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .map_err(failed)?;
        flock(&file, FlockOperation::LockExclusive).map_err(|error| failed(error.into()))?;
        Ok(Lock { _file: file })
    }

    /// Sin historial todavía no hay nada que proteger, y leer no lo crea.
    pub(super) fn read_lock(&self) -> Result<ReadLock, HistoryError> {
        let path = self.dir.join("lock");
        let failed = |source| HistoryError::io("bloquear", &path, source);
        let file = match File::open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(ReadLock { _file: None });
            }
            Err(source) => return Err(failed(source)),
        };
        flock(&file, FlockOperation::LockShared).map_err(|error| failed(error.into()))?;
        Ok(ReadLock { _file: Some(file) })
    }

    pub(super) fn read_index(&self) -> Result<Index, HistoryError> {
        let path = self.index_path();
        match fs::read(&path) {
            Ok(bytes) => {
                decode_index(&bytes).ok_or_else(|| HistoryError::corrupt(&path, "índice dañado"))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Index::default()),
            Err(source) => Err(HistoryError::io("leer", &path, source)),
        }
    }

    pub(super) fn write_index(&self, _lock: &Lock, index: &Index) -> Result<(), HistoryError> {
        let path = self.index_path();
        let bytes = encode_index(index);
        write_atomic(&path, |out| out.write_all(&bytes))
            .map_err(|error| HistoryError::File(Box::new(error)))
    }

    pub(super) fn corrupt_log(&self, id: u64, corrupt: Corrupt) -> HistoryError {
        HistoryError::corrupt(&self.log_path(id), corrupt.0)
    }

    pub(super) fn read_log(&self, id: u64) -> Result<NoteLog, HistoryError> {
        let path = self.log_path(id);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(source) => return Err(HistoryError::io("leer", &path, source)),
        };
        let log = NoteLog::parse(&bytes).map_err(|corrupt| self.corrupt_log(id, corrupt))?;
        if log.valid_len() < bytes.len() as u64 {
            log::warn!(
                "historial {}: se ignoran {} bytes de un guardado cortado",
                path.display(),
                bytes.len() as u64 - log.valid_len()
            );
        }
        Ok(log)
    }

    /// Escribe `frames` donde termina la parte válida del log, pisando la
    /// cola de un guardado cortado si la hubiera.
    pub(super) fn append_log(
        &self,
        _lock: &Lock,
        id: u64,
        valid_len: u64,
        frames: &[u8],
    ) -> Result<(), HistoryError> {
        let path = self.log_path(id);
        let write = || -> io::Result<()> {
            let mut file = OpenOptions::new()
                .create(true)
                .truncate(false)
                .write(true)
                .open(&path)?;
            file.set_len(valid_len)?;
            file.seek(SeekFrom::Start(valid_len))?;
            file.write_all(frames)?;
            file.sync_data()?;
            // Un log nuevo: que su entrada en `notes/` llegue al disco antes que
            // el índice que lo nombra.
            if valid_len == 0 {
                File::open(self.notes_dir())?.sync_all()?;
            }
            Ok(())
        };
        write().map_err(|source| HistoryError::io("guardar", &path, source))
    }

    pub(super) fn replace_log(
        &self,
        _lock: &Lock,
        id: u64,
        bytes: &[u8],
    ) -> Result<(), HistoryError> {
        write_atomic(&self.log_path(id), |out| out.write_all(bytes))
            .map_err(|error| HistoryError::File(Box::new(error)))
    }

    /// Aparta un log dañado como `<id>.log.damaged` (o `.damaged.1`, `.2`…
    /// si ya hay otro): no se pierde, pero ya no es parte del historial y su
    /// id puede empezar uno nuevo.
    pub(super) fn set_aside(&self, _lock: &Lock, id: u64) -> Result<(), HistoryError> {
        let path = self.log_path(id);
        let failed = |source| HistoryError::io("apartar", &path, source);
        for copy in 0_u32.. {
            let mut aside = path.clone().into_os_string();
            aside.push(".damaged");
            if copy > 0 {
                aside.push(format!(".{copy}"));
            }
            // Un enlace nuevo nunca pisa una copia anterior, a diferencia de `rename`.
            match fs::hard_link(&path, &aside) {
                Ok(()) => return fs::remove_file(&path).map_err(failed),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
                Err(source) => return Err(failed(source)),
            }
        }
        Ok(())
    }

    pub(super) fn remove_log(&self, _lock: &Lock, id: u64) -> Result<(), HistoryError> {
        let path = self.log_path(id);
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(HistoryError::io("borrar", &path, source)),
        }
    }

    /// Los ids de los logs que hay en disco, estén o no en el índice.
    pub(super) fn log_ids(&self) -> Result<Vec<u64>, HistoryError> {
        let dir = self.notes_dir();
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => return Err(HistoryError::io("leer", &dir, source)),
        };
        let mut ids = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|source| HistoryError::io("leer", &dir, source))?;
            let name = entry.file_name();
            let id = name
                .to_str()
                .and_then(|name| name.strip_suffix(".log"))
                .and_then(|hex| u64::from_str_radix(hex, 16).ok());
            ids.extend(id);
        }
        Ok(ids)
    }

    fn index_path(&self) -> PathBuf {
        self.dir.join("index")
    }

    fn notes_dir(&self) -> PathBuf {
        self.dir.join("notes")
    }

    fn log_path(&self, id: u64) -> PathBuf {
        self.notes_dir().join(format!("{id:016x}.log"))
    }
}

/// `[magic][próximo id]` y por historia `[tipo][id][borrada][ruta]`, con un
/// checksum al final: el índice se reemplaza entero, así que no hay colas
/// cortadas.
fn encode_index(index: &Index) -> Vec<u8> {
    let mut out = INDEX_MAGIC.to_vec();
    put_var(&mut out, index.next_id);
    let notes = index.notes.iter().map(|(path, entry)| (NOTE, path, entry));
    let set_aside = index
        .set_aside
        .iter()
        .map(|(path, entry)| (SET_ASIDE, path, entry));
    for (kind, path, entry) in notes.chain(set_aside) {
        out.push(kind);
        put_var(&mut out, entry.id);
        out.extend_from_slice(&entry.deleted.unwrap_or(NO_DELETION).to_le_bytes());
        put_bytes(&mut out, path.as_str().as_bytes());
    }
    let check = blake3::hash(&out);
    out.extend_from_slice(&check.as_bytes()[..8]);
    out
}

fn decode_index(bytes: &[u8]) -> Option<Index> {
    let (body, check) = bytes.split_at_checked(bytes.len().checked_sub(8)?)?;
    if blake3::hash(body).as_bytes()[..8] != *check {
        return None;
    }
    let mut reader = Reader::new(body.strip_prefix(INDEX_MAGIC)?);
    let mut index = Index {
        next_id: reader.var()?,
        ..Index::default()
    };
    while !reader.is_empty() {
        let kind = reader.u8()?;
        let id = reader.var()?;
        let deleted = i64::from_le_bytes(reader.array()?);
        let path = VaultPath::new(std::str::from_utf8(reader.bytes()?).ok()?).ok()?;
        let deleted = (deleted != NO_DELETION).then_some(deleted);
        let entry = IndexEntry { id, deleted };
        match kind {
            NOTE => {
                index.notes.insert(path, entry);
            }
            SET_ASIDE if deleted.is_some() => index.set_aside.push((path, entry)),
            _ => return None,
        }
    }
    Some(index)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn el_indice_vuelve_igual() {
        let mut index = Index::default();
        index.allocate(VaultPath::new("a/ñandú.md").unwrap());
        let borrada = index.allocate(VaultPath::new("b.md").unwrap());
        index.notes.insert(
            VaultPath::new("b.md").unwrap(),
            IndexEntry {
                deleted: Some(-5),
                ..borrada
            },
        );
        let vieja = index.allocate(VaultPath::new("c.md").unwrap());
        index.notes.remove(&VaultPath::new("c.md").unwrap());
        index.set_aside.push((
            VaultPath::new("c.md").unwrap(),
            IndexEntry {
                deleted: Some(7),
                ..vieja
            },
        ));
        let bytes = encode_index(&index);
        assert_eq!(decode_index(&bytes), Some(index));
    }

    #[test]
    fn un_indice_tocado_no_se_lee() {
        let mut bytes = encode_index(&Index::default());
        bytes[8] ^= 1;
        assert_eq!(decode_index(&bytes), None);
        assert_eq!(decode_index(b"corto"), None);
    }
}
