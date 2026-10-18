//! Una versión guardada como cambio sobre otra: copiar tramos de la base o
//! insertar texto nuevo. Sale del diff por líneas, así que una edición chica
//! en una nota grande cuesta lo que la línea tocada.

use super::bytes::{Reader, put_bytes, put_var};
use super::diff::{ChunkKind, line_chunks};

const COPY: u8 = 0;
const INSERT: u8 = 1;

pub(super) fn encode(base: &str, text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for chunk in line_chunks(base, text) {
        match chunk.kind {
            ChunkKind::Equal => {
                out.push(COPY);
                put_var(&mut out, chunk.old.start as u64);
                put_var(&mut out, chunk.old.len() as u64);
            }
            ChunkKind::Insert | ChunkKind::Replace => {
                out.push(INSERT);
                put_bytes(&mut out, &text.as_bytes()[chunk.new]);
            }
            ChunkKind::Delete => {}
        }
    }
    out
}

/// Lo que da `delta` sobre `base`, o `None` si el delta está dañado o no da
/// exactamente `len` bytes. Va en bytes: quien reconstruye una cadena de
/// deltas valida el UTF-8 una vez, al final, y no en cada paso.
pub(super) fn apply(base: &[u8], delta: &[u8], len: usize) -> Option<Vec<u8>> {
    // Sin reservar `len` de entrada: en un registro dañado puede ser enorme.
    let mut out = Vec::new();
    let mut reader = Reader::new(delta);
    while !reader.is_empty() {
        let piece = match reader.u8()? {
            COPY => {
                let start = reader.usize()?;
                let end = start.checked_add(reader.usize()?)?;
                base.get(start..end)?
            }
            INSERT => reader.bytes()?,
            _ => return None,
        };
        if piece.len() > len - out.len() {
            return None;
        }
        out.extend_from_slice(piece);
    }
    (out.len() == len).then_some(out)
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn una_linea_cambiada_cuesta_esa_linea() {
        let base = "línea de la nota\n".repeat(5_000);
        let at = base.len() / 2 - 3;
        let text = format!("{}cambio {}", &base[..at], &base[at..]);
        let delta = encode(&base, &text);
        assert!(delta.len() < 64, "{} bytes", delta.len());
        assert_eq!(
            apply(base.as_bytes(), &delta, text.len()),
            Some(text.into_bytes())
        );
    }

    #[test]
    fn un_delta_danado_no_se_aplica() {
        assert_eq!(apply(b"abc", &[COPY, 2, 5], 0), None);
        assert_eq!(apply(b"abc", &[7], 0), None);
        assert_eq!(apply(b"abc", &[INSERT, 5, b'x'], 0), None);
        let copy_all = [COPY, 0, 3];
        assert_eq!(apply(b"abc", &copy_all, 3), Some(b"abc".to_vec()));
        assert_eq!(apply(b"abc", &copy_all, 2), None, "da más de lo dicho");
        assert_eq!(apply(b"abc", &copy_all, 4), None, "da menos de lo dicho");
    }

    proptest! {
        #[test]
        fn aplicar_el_delta_da_el_texto(base in "[ab\nñ]{0,60}", text in "[ab\nñ]{0,60}") {
            let delta = encode(&base, &text);
            prop_assert_eq!(apply(base.as_bytes(), &delta, text.len()), Some(text.into_bytes()));
        }
    }
}
