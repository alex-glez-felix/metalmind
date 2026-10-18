//! El input handler de GPUI habla en unidades UTF-16; el resto, en bytes.

pub fn len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// Byte donde empieza el carácter que contiene la unidad `utf16`; pasado el
/// final, el largo del texto.
pub fn to_byte(text: &str, utf16: usize) -> usize {
    let mut units = 0;
    for (byte, c) in text.char_indices() {
        units += c.len_utf16();
        if units > utf16 {
            return byte;
        }
    }
    text.len()
}

/// Unidades UTF-16 antes del byte `byte`, que debe ser frontera de carácter.
pub fn from_byte(text: &str, byte: usize) -> usize {
    len(&text[..byte.min(text.len())])
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "añ😀b";

    #[test]
    fn largo_en_unidades() {
        assert_eq!(len(TEXT), 5);
        assert_eq!(len(""), 0);
    }

    #[test]
    fn de_utf16_a_bytes() {
        let cases = [(0, 0), (1, 1), (2, 3), (3, 3), (4, 7), (5, 8), (9, 8)];
        for (utf16, byte) in cases {
            assert_eq!(to_byte(TEXT, utf16), byte, "utf16={utf16}");
        }
    }

    #[test]
    fn de_bytes_a_utf16() {
        let cases = [(0, 0), (1, 1), (3, 2), (7, 4), (8, 5), (99, 5)];
        for (byte, utf16) in cases {
            assert_eq!(from_byte(TEXT, byte), utf16, "byte={byte}");
        }
    }
}
