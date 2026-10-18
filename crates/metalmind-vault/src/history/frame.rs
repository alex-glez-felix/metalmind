//! Un marco del log: `[largo][checksum][tipo][datos]`. El checksum separa
//! un marco entero de uno a medio escribir o dañado.

use super::bytes::{Reader, put_var};

/// `[largo][checksum 4][tipo][datos]`; el largo cuenta tipo y datos.
pub(super) fn push_frame(out: &mut Vec<u8>, kind: u8, payload: &[u8]) {
    put_var(out, payload.len() as u64 + 1);
    out.extend_from_slice(&checksum(kind, payload));
    out.push(kind);
    out.extend_from_slice(payload);
}

pub(super) enum Frame<'a> {
    Valid {
        kind: u8,
        payload: &'a [u8],
        next: usize,
    },
    /// Completo pero con el checksum mal; `next` es donde acabaría.
    Damaged { next: usize },
    /// No cabe en lo que queda del archivo, o no queda nada.
    Torn,
}

/// El marco que empieza en `offset`.
pub(super) fn frame_at(bytes: &[u8], offset: usize) -> Frame<'_> {
    let Some(rest) = bytes.get(offset..) else {
        return Frame::Torn;
    };
    let mut reader = Reader::new(rest);
    let Some((check, body)) = reader.usize().and_then(|len| {
        let check: [u8; 4] = reader.array()?;
        Some((check, reader.take(len)?))
    }) else {
        return Frame::Torn;
    };
    let next = offset + rest.len() - reader.rest().len();
    match body.split_first() {
        Some((&kind, payload)) if checksum(kind, payload) == check => Frame::Valid {
            kind,
            payload,
            next,
        },
        _ => Frame::Damaged { next },
    }
}

fn checksum(kind: u8, payload: &[u8]) -> [u8; 4] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&[kind]);
    hasher.update(payload);
    let hash = hasher.finalize();
    let mut check = [0; 4];
    check.copy_from_slice(&hash.as_bytes()[..4]);
    check
}
