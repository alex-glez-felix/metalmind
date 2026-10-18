//! Lo mínimo para leer y escribir los formatos binarios del historial:
//! enteros LEB128 y tramos con su largo delante.

pub(super) fn put_var(out: &mut Vec<u8>, mut value: u64) {
    loop {
        let low = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(low);
            return;
        }
        out.push(low | 0x80);
    }
}

pub(super) fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    put_var(out, bytes.len() as u64);
    out.extend_from_slice(bytes);
}

/// Lee de un tramo de bytes; cada método da `None` si no alcanza o el valor
/// no tiene sentido, y quien llama lo trata como un archivo dañado.
pub(super) struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    pub(super) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Lo que queda, sin consumirlo.
    pub(super) fn rest(&self) -> &'a [u8] {
        self.bytes
    }

    pub(super) fn take(&mut self, len: usize) -> Option<&'a [u8]> {
        if len > self.bytes.len() {
            return None;
        }
        let (taken, rest) = self.bytes.split_at(len);
        self.bytes = rest;
        Some(taken)
    }

    pub(super) fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
        self.take(N)?.try_into().ok()
    }

    pub(super) fn u8(&mut self) -> Option<u8> {
        Some(self.array::<1>()?[0])
    }

    pub(super) fn var(&mut self) -> Option<u64> {
        let mut value = 0_u64;
        for shift in (0..64).step_by(7) {
            let byte = self.u8()?;
            let bits = u64::from(byte & 0x7f);
            if bits.leading_zeros() < shift {
                return None;
            }
            value |= bits << shift;
            if byte & 0x80 == 0 {
                return Some(value);
            }
        }
        None
    }

    pub(super) fn usize(&mut self) -> Option<usize> {
        usize::try_from(self.var()?).ok()
    }

    pub(super) fn bytes(&mut self) -> Option<&'a [u8]> {
        let len = self.usize()?;
        self.take(len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_enteros_vuelven_iguales() {
        let values = [0, 1, 127, 128, 300, u64::from(u32::MAX), u64::MAX];
        let mut out = Vec::new();
        for value in values {
            put_var(&mut out, value);
        }
        let mut reader = Reader::new(&out);
        for value in values {
            assert_eq!(reader.var(), Some(value));
        }
        assert!(reader.is_empty());
    }

    #[test]
    fn un_entero_cortado_o_demasiado_largo_no_se_lee() {
        assert_eq!(Reader::new(&[0x80]).var(), None);
        assert_eq!(Reader::new(&[0xff; 11]).var(), None);
    }
}
