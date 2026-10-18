//! El único paso entre el contenido de una nota y los bytes del historial en
//! disco. Las notas cifradas (06 §4) entran aquí: un `Codec` nuevo que cifra
//! después de comprimir, para que ni snapshots ni deltas queden en claro.

use miniz_oxide::deflate::compress_to_vec;
use miniz_oxide::inflate::decompress_to_vec_with_limit;

/// Nivel de deflate: medido en 03 §4, del 6 al 9 casi no gana y tarda más.
const LEVEL: u8 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Codec {
    Raw,
    Deflate,
}

impl Codec {
    pub(super) fn tag(self) -> u8 {
        match self {
            Self::Raw => 0,
            Self::Deflate => 1,
        }
    }

    pub(super) fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::Raw),
            1 => Some(Self::Deflate),
            _ => None,
        }
    }
}

/// Los bytes que van a disco: comprimidos si así ocupan menos.
pub(super) fn seal(payload: Vec<u8>) -> (Codec, Vec<u8>) {
    let compressed = compress_to_vec(&payload, LEVEL);
    if compressed.len() < payload.len() {
        (Codec::Deflate, compressed)
    } else {
        (Codec::Raw, payload)
    }
}

/// Lo que se guardó con [`seal`], que medía `len` bytes; `None` si no cuadra.
pub(super) fn open(codec: Codec, stored: &[u8], len: usize) -> Option<Vec<u8>> {
    let payload = match codec {
        Codec::Raw => stored.to_vec(),
        Codec::Deflate => decompress_to_vec_with_limit(stored, len).ok()?,
    };
    (payload.len() == len).then_some(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comprime_solo_si_gana() {
        let text = "una línea que se repite\n".repeat(100).into_bytes();
        let (codec, stored) = seal(text.clone());
        assert_eq!(codec, Codec::Deflate);
        assert!(stored.len() < text.len() / 10);
        assert_eq!(open(codec, &stored, text.len()), Some(text));

        assert_eq!(seal(b"ab".to_vec()), (Codec::Raw, b"ab".to_vec()));
    }

    #[test]
    fn un_largo_que_no_cuadra_es_un_registro_danado() {
        let (codec, stored) = seal(vec![b'a'; 1000]);
        assert_eq!(open(codec, &stored, 999), None);
        assert_eq!(open(Codec::Raw, b"abc", 4), None);
    }
}
