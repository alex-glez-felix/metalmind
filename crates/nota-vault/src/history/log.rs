//! El historial de una nota en disco: un archivo al que solo se le agregan
//! marcos. Un marco `RECORD` guarda un contenido, entero (snapshot) o como
//! delta sobre otro registro; uno `VERSION` dice qué registro tenía la nota
//! en un momento. Cada marco lleva su largo y un checksum, así que un
//! guardado cortado a la mitad deja una cola que se ignora y se pisa.

use super::ContentHash;
use super::bytes::{Reader, put_var};
use super::codec::{self, Codec};
use super::delta;
use super::frame::{Frame, frame_at, push_frame};

mod contents;

const MAGIC: &[u8; 8] = b"notahis1";
const RECORD: u8 = 1;
const VERSION: u8 = 2;
/// Cuántos deltas puede haber entre un registro y su snapshot: acota lo que
/// cuesta reconstruir una versión.
const MAX_DEPTH: u32 = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Corrupt(pub &'static str);

pub(super) const DAMAGED: Corrupt = Corrupt("registro dañado");

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Entry {
    pub seq: u64,
    /// Milisegundos desde 1970, UTC.
    pub at: i64,
    pub hash: ContentHash,
    pub len: usize,
    pub record: usize,
}

struct Record {
    codec: Codec,
    /// `None` en un snapshot.
    base: Option<usize>,
    payload_len: usize,
    text_len: usize,
    stored: Vec<u8>,
    /// El snapshot del que cuelga, cuántos deltas hay hasta él y cuántos
    /// bytes ocupan.
    root: usize,
    depth: u32,
    chain_bytes: usize,
}

#[derive(Default)]
pub(super) struct NoteLog {
    records: Vec<Record>,
    entries: Vec<Entry>,
    /// Hasta dónde el archivo es válido; lo que sigue es una cola cortada.
    valid_len: u64,
}

impl Record {
    fn open(&self) -> Result<Vec<u8>, Corrupt> {
        codec::open(self.codec, &self.stored, self.payload_len).ok_or(DAMAGED)
    }
}

impl NoteLog {
    pub(super) fn parse(bytes: &[u8]) -> Result<Self, Corrupt> {
        let mut log = Self::default();
        if bytes.len() < MAGIC.len() {
            // Vacío, o un primer guardado que se cortó antes de terminar la cabecera.
            return if MAGIC.starts_with(bytes) {
                Ok(log)
            } else {
                Err(Corrupt("no es un historial de nota"))
            };
        }
        if !bytes.starts_with(MAGIC) {
            return Err(Corrupt("no es un historial de nota"));
        }
        let mut offset = MAGIC.len();
        loop {
            match frame_at(bytes, offset) {
                Frame::Valid {
                    kind,
                    payload,
                    next,
                } => {
                    match kind {
                        RECORD => log.read_record(payload)?,
                        VERSION => log.read_entry(payload)?,
                        _ => return Err(Corrupt("marco desconocido")),
                    }
                    offset = next;
                }
                // Un guardado cortado solo daña el final; si detrás del marco
                // malo sigue uno bueno, el daño es otro y no se pisa nada.
                Frame::Damaged { next } => {
                    if matches!(frame_at(bytes, next), Frame::Valid { .. }) {
                        return Err(Corrupt("marco dañado en medio del historial"));
                    }
                    break;
                }
                Frame::Torn => break,
            }
        }
        log.valid_len = offset as u64;
        Ok(log)
    }

    pub(super) fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub(super) fn valid_len(&self) -> u64 {
        self.valid_len
    }

    pub(super) fn content(&self, record: usize) -> Result<String, Corrupt> {
        let mut chain = Vec::new();
        let mut at = record;
        loop {
            let current = self.record(at)?;
            chain.push(current);
            match current.base {
                Some(base) => at = base,
                None => break,
            }
        }
        let mut bytes = Vec::new();
        for current in chain.into_iter().rev() {
            let payload = current.open()?;
            bytes = match current.base {
                None if payload.len() == current.text_len => payload,
                None => return Err(DAMAGED),
                Some(_) => delta::apply(&bytes, &payload, current.text_len).ok_or(DAMAGED)?,
            };
        }
        String::from_utf8(bytes).map_err(|_| DAMAGED)
    }

    /// Pasa a `visit` el contenido de cada registro de `wanted`, en orden.
    /// Va como `contents`, así que no tiene todas las versiones en memoria.
    pub(super) fn for_each_content(
        &self,
        wanted: &[usize],
        mut visit: impl FnMut(&str) -> Result<(), Corrupt>,
    ) -> Result<(), Corrupt> {
        let mut contents = self.contents(wanted)?;
        while let Some(text) = contents.next_text()? {
            visit(&text)?;
        }
        Ok(())
    }

    /// Agrega `text` como versión nueva, salvo que sea igual a la última, y
    /// devuelve su número y los bytes que hay que escribir en `valid_len`.
    /// `latest`, si viene, es el contenido de la última versión: ahorra
    /// reconstruirlo para calcular el delta.
    pub(super) fn append(
        &mut self,
        text: &str,
        at: i64,
        latest: Option<&str>,
    ) -> Result<Option<(u64, Vec<u8>)>, Corrupt> {
        let hash = ContentHash::of(text);
        if self.entries.last().is_some_and(|last| last.hash == hash) {
            return Ok(None);
        }
        let seq = match self.entries.last() {
            Some(last) => last.seq.checked_add(1).ok_or(Corrupt("versión dañada"))?,
            None => 0,
        };
        let frames = self.push(seq, at, hash, text, latest)?;
        Ok(Some((seq, frames)))
    }

    /// Agrega una versión con número `seq` aunque sea igual a la anterior;
    /// es lo que se usa para rehacer un historial al podar.
    /// `previous` es el texto de la última versión agregada, para no
    /// reconstruirlo desde su snapshot en cada una.
    pub(super) fn push_version(
        &mut self,
        seq: u64,
        at: i64,
        text: &str,
        previous: Option<&str>,
    ) -> Result<Vec<u8>, Corrupt> {
        self.push(seq, at, ContentHash::of(text), text, previous)
    }

    fn push(
        &mut self,
        seq: u64,
        at: i64,
        hash: ContentHash,
        text: &str,
        latest: Option<&str>,
    ) -> Result<Vec<u8>, Corrupt> {
        let mut out = if self.valid_len == 0 {
            MAGIC.to_vec()
        } else {
            Vec::new()
        };
        // Mismo contenido que una versión anterior: comparte su registro.
        let shared = self.entries.iter().rev().find(|entry| entry.hash == hash);
        let record = if let Some(same) = shared {
            same.record
        } else {
            let payload = self.encode_record(text, latest)?;
            push_frame(&mut out, RECORD, &payload);
            self.read_record(&payload)?;
            self.records.len() - 1
        };
        let mut payload = Vec::new();
        put_var(&mut payload, seq);
        payload.extend_from_slice(&at.to_le_bytes());
        payload.extend_from_slice(hash.as_bytes());
        put_var(&mut payload, text.len() as u64);
        put_var(&mut payload, record as u64);
        push_frame(&mut out, VERSION, &payload);
        self.read_entry(&payload)?;
        self.valid_len += out.len() as u64;
        Ok(out)
    }

    /// Delta sobre la versión anterior mientras la cadena hasta su snapshot
    /// no pese más que el snapshot mismo; si no, un snapshot nuevo. Así el
    /// historial ocupa a lo sumo el doble que solo deltas y reconstruir una
    /// versión lee a lo sumo el doble de su snapshot.
    fn encode_record(&self, text: &str, latest: Option<&str>) -> Result<Vec<u8>, Corrupt> {
        if let Some(last) = self.entries.last() {
            let base = self.record(last.record)?;
            if base.depth < MAX_DEPTH {
                let delta = match latest {
                    Some(latest) => delta::encode(latest, text),
                    None => delta::encode(&self.content(last.record)?, text),
                };
                let payload = record_payload(Some(last.record), text.len(), delta);
                let stored = payload.len();
                let budget = self.record(base.root)?.stored.len();
                if base.chain_bytes + stored <= budget {
                    return Ok(payload);
                }
            }
        }
        Ok(record_payload(None, text.len(), text.as_bytes().to_vec()))
    }

    fn record(&self, index: usize) -> Result<&Record, Corrupt> {
        self.records
            .get(index)
            .ok_or(Corrupt("registro inexistente"))
    }

    fn read_record(&mut self, payload: &[u8]) -> Result<(), Corrupt> {
        let damaged = DAMAGED;
        let mut reader = Reader::new(payload);
        let codec = reader.u8().and_then(Codec::from_tag).ok_or(damaged)?;
        let base = match reader.usize().ok_or(damaged)? {
            0 => None,
            plus_one => Some(plus_one - 1),
        };
        let payload_len = reader.usize().ok_or(damaged)?;
        let text_len = reader.usize().ok_or(damaged)?;
        let stored = reader.rest().to_vec();
        let index = self.records.len();
        let (root, depth, chain_bytes) = match base {
            None => (index, 0, 0),
            Some(base) if base < index => {
                let parent = &self.records[base];
                (
                    parent.root,
                    parent.depth + 1,
                    parent.chain_bytes + stored.len(),
                )
            }
            Some(_) => return Err(damaged),
        };
        self.records.push(Record {
            codec,
            base,
            payload_len,
            text_len,
            stored,
            root,
            depth,
            chain_bytes,
        });
        Ok(())
    }

    fn read_entry(&mut self, payload: &[u8]) -> Result<(), Corrupt> {
        let damaged = Corrupt("versión dañada");
        let mut reader = Reader::new(payload);
        let entry = Entry {
            seq: reader.var().ok_or(damaged)?,
            at: i64::from_le_bytes(reader.array().ok_or(damaged)?),
            hash: ContentHash(reader.array().ok_or(damaged)?),
            len: reader.usize().ok_or(damaged)?,
            record: reader.usize().ok_or(damaged)?,
        };
        let record = self.records.get(entry.record).ok_or(damaged)?;
        // Los números crecen y siempre cabe uno más; el largo es el del registro.
        let after_last = self.entries.last().is_none_or(|last| entry.seq > last.seq);
        if !reader.is_empty()
            || entry.len != record.text_len
            || !after_last
            || entry.seq == u64::MAX
        {
            return Err(damaged);
        }
        self.entries.push(entry);
        Ok(())
    }
}

/// `[codec][base + 1, o 0 si es snapshot][largo sin comprimir][largo del texto][datos]`.
fn record_payload(base: Option<usize>, text_len: usize, raw: Vec<u8>) -> Vec<u8> {
    let raw_len = raw.len();
    let (codec, stored) = codec::seal(raw);
    let mut payload = vec![codec.tag()];
    put_var(&mut payload, base.map_or(0, |base| base as u64 + 1));
    put_var(&mut payload, raw_len as u64);
    put_var(&mut payload, text_len as u64);
    payload.extend(stored);
    payload
}

#[cfg(test)]
#[path = "log_tests.rs"]
mod tests;
