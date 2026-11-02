//! El archivo de la caché: un encabezado y bloques de notas en `postcard`,
//! cada uno con su blake3. Se escribe entero y atómico, y se lee entero: un
//! byte que no cuadre descarta todo, porque lo guardado son rangos sobre el
//! texto de la nota y uno malo haría fallar a quien lo use después.
//!
//! ```text
//! MAGIC | versión (32 bytes) | cuántos bloques (u32)
//! blake3 (32 bytes) | largo (u32) | Vec<NoteRecord>     una vez por bloque
//! ```

use std::borrow::Cow;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

use metalmind_syntax::Property;

use super::{CachedNote, Stamp};
use crate::file::{FileError, write_atomic};
use crate::history::ContentHash;
use crate::links::IndexedLink;
use crate::note_scan::NoteScan;
use crate::parallel::in_parallel;
use crate::tags::NoteTags;

const MAGIC: &[u8; 8] = b"MMNOTES\n";
/// Sube cuando cambia lo de arriba o [`NoteRecord`]. Lo que cambie el scanner
/// sin cambiar los tipos lo atrapa [`version`] con [`SAMPLE`].
const FORMAT: u32 = 1;
/// Lee y verifica por bloques, en paralelo, sin que un bloque pese demasiado.
const NOTES_PER_BLOCK: usize = 1024;
const HASH_LEN: usize = 32;
const LEN_BYTES: usize = 4;

/// Una nota con todo lo que el scanner sabe sacar: si cambia lo que saca de
/// aquí, lo guardado por otra versión ya no sirve.
const SAMPLE: &str = "---\n\
tags: [uno, \"#dos\", \"tr\\u0065s\"]\n\
aliases: [otro]\n\
fecha: {año: 2026, listas: [a, b]}\n\
---\n\
# Título #cuatro\n\
[[nota]] ![[imagen.png|200]] [[nota#Sección]] [[nota#^bloque]] [[#Aquí]]\n\
[md](<mi nota.md#Sec%201>) ![img](a%20b.png) `[[código]]`\n\
- [ ] #cinco/seis $$x$$\n";

/// Una nota en el archivo. `Cow` para escribir sin copiar lo de los índices
/// y leer con los mismos tipos: el orden de los campos es parte del formato.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct NoteRecord<'a> {
    pub path: Cow<'a, str>,
    pub stamp: Option<Stamp>,
    pub hash: ContentHash,
    pub links: Cow<'a, [IndexedLink]>,
    pub tags: Cow<'a, NoteTags>,
    pub properties: Cow<'a, [Property]>,
}

impl NoteRecord<'_> {
    fn into_cached(self) -> (String, CachedNote) {
        let note = CachedNote {
            stamp: self.stamp,
            hash: self.hash,
            scan: NoteScan {
                links: self.links.into_owned(),
                tags: self.tags.into_owned(),
                properties: self.properties.into_owned(),
            },
        };
        (self.path.into_owned(), note)
    }
}

/// Por qué no sirve un archivo de caché.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub(super) enum Unusable {
    #[error("no es una caché de notas")]
    NotACache,
    #[error("es de otra versión")]
    OtherVersion,
    #[error("está corrupta")]
    Corrupt,
}

/// Las notas de `file`; ninguna si no existe o no sirve, y entonces se
/// reconstruye de los `.md`.
pub(super) fn load(file: &Path) -> HashMap<String, CachedNote> {
    let bytes = match fs::read(file) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return HashMap::new(),
        Err(error) => {
            log::warn!("no se pudo leer la caché {}: {error}", file.display());
            return HashMap::new();
        }
    };
    parse(&bytes).unwrap_or_else(|reason| {
        log::warn!("se descarta la caché {}: {reason}", file.display());
        HashMap::new()
    })
}

pub(super) fn parse(bytes: &[u8]) -> Result<HashMap<String, CachedNote>, Unusable> {
    let blocks = blocks(bytes)?;
    let decoded = in_parallel(blocks, decode_block);
    let mut notes = HashMap::new();
    for block in decoded {
        notes.extend(block.ok_or(Unusable::Corrupt)?);
    }
    Ok(notes)
}

/// Un bloque con el hash que dice tener, sin mirarlo todavía.
#[derive(Clone, Copy)]
struct Block<'b> {
    hash: &'b [u8],
    payload: &'b [u8],
}

fn blocks(bytes: &[u8]) -> Result<Vec<Block<'_>>, Unusable> {
    let rest = bytes.strip_prefix(MAGIC).ok_or(Unusable::NotACache)?;
    let (found, rest) = rest.split_at_checked(HASH_LEN).ok_or(Unusable::Corrupt)?;
    if found != version().as_slice() {
        return Err(Unusable::OtherVersion);
    }
    let (count, mut rest) = take_len(rest).ok_or(Unusable::Corrupt)?;
    let mut blocks = Vec::new();
    for _ in 0..count {
        let (hash, after_hash) = rest.split_at_checked(HASH_LEN).ok_or(Unusable::Corrupt)?;
        let (len, after_len) = take_len(after_hash).ok_or(Unusable::Corrupt)?;
        let (payload, after) = after_len.split_at_checked(len).ok_or(Unusable::Corrupt)?;
        blocks.push(Block { hash, payload });
        rest = after;
    }
    if rest.is_empty() {
        Ok(blocks)
    } else {
        Err(Unusable::Corrupt)
    }
}

fn take_len(bytes: &[u8]) -> Option<(usize, &[u8])> {
    let (len, rest) = bytes.split_first_chunk::<LEN_BYTES>()?;
    Some((usize::try_from(u32::from_le_bytes(*len)).ok()?, rest))
}

fn decode_block(block: Block<'_>) -> Option<Vec<(String, CachedNote)>> {
    if blake3::hash(block.payload).as_bytes().as_slice() != block.hash {
        return None;
    }
    let records: Vec<NoteRecord<'_>> = postcard::from_bytes(block.payload).ok()?;
    Some(records.into_iter().map(NoteRecord::into_cached).collect())
}

pub(super) fn save(file: &Path, records: &[NoteRecord<'_>]) -> Result<(), FileError> {
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent).map_err(|source| FileError::Write {
            path: parent.to_owned(),
            source,
        })?;
    }
    write_atomic(file, |out| write_records(out, records))
}

fn write_records(out: &mut dyn Write, records: &[NoteRecord<'_>]) -> io::Result<()> {
    out.write_all(MAGIC)?;
    out.write_all(&version())?;
    write_len(out, records.len().div_ceil(NOTES_PER_BLOCK))?;
    for block in records.chunks(NOTES_PER_BLOCK) {
        let payload = postcard::to_allocvec(block).map_err(io::Error::other)?;
        out.write_all(blake3::hash(&payload).as_bytes())?;
        write_len(out, payload.len())?;
        out.write_all(&payload)?;
    }
    Ok(())
}

fn write_len(out: &mut dyn Write, len: usize) -> io::Result<()> {
    let len = u32::try_from(len).map_err(io::Error::other)?;
    out.write_all(&len.to_le_bytes())
}

/// Identifica a quién puede leer un archivo: el formato, el release y lo que
/// el scanner de este binario saca de [`SAMPLE`]. Cambiar el scanner sin
/// acordarse de nada igual descarta lo guardado.
pub(super) fn version() -> [u8; HASH_LEN] {
    let scan = NoteScan::of(SAMPLE);
    let sample = NoteRecord {
        path: Cow::Borrowed(""),
        stamp: None,
        hash: ContentHash::of(SAMPLE),
        links: Cow::Owned(scan.links),
        tags: Cow::Owned(scan.tags),
        properties: Cow::Owned(scan.properties),
    };
    let mut hasher = blake3::Hasher::new();
    hasher.update(&FORMAT.to_le_bytes());
    hasher.update(env!("CARGO_PKG_VERSION").as_bytes());
    // Serializar a memoria solo falla por un tipo inválido, y `save` también lo notaría.
    if let Ok(encoded) = postcard::to_allocvec(&sample) {
        hasher.update(&encoded);
    }
    *hasher.finalize().as_bytes()
}
