//! Un offset del texto con su carácter y su línea, sacados de un solo
//! recorrido del rope: cada recorrido cuesta lo mismo que contar los
//! caracteres de un trozo entero, así que las ediciones los ahorran.

use ropey::Rope;

use crate::anchor::Point;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Spot {
    pub byte: usize,
    /// En caracteres, como los cuenta el rope para editar.
    pub char: usize,
    pub point: Point,
}

impl Spot {
    /// `None` si `byte` se sale del texto o parte un carácter.
    pub fn of(rope: &Rope, byte: usize) -> Option<Self> {
        let (chunk, chunk_byte, chunk_char, chunk_line) = rope.get_chunk_at_byte(byte)?;
        let before = chunk.get(..byte - chunk_byte)?;
        let (line, line_start) = match before.rfind('\n') {
            Some(newline) => (
                chunk_line + newlines(&before[..newline]) + 1,
                chunk_byte + newline + 1,
            ),
            // La línea empieza en un trozo anterior.
            None => (chunk_line, rope.line_to_byte(chunk_line)),
        };
        Some(Self {
            byte,
            char: chunk_char + before.chars().count(),
            point: Point::new(line, byte - line_start),
        })
    }
}

/// Si `byte` cae en el texto y en el inicio de un carácter.
pub(crate) fn is_char_boundary(rope: &Rope, byte: usize) -> bool {
    rope.get_chunk_at_byte(byte)
        .is_some_and(|(chunk, chunk_byte, ..)| chunk.is_char_boundary(byte - chunk_byte))
}

fn newlines(text: &str) -> usize {
    text.bytes().filter(|&byte| byte == b'\n').count()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lo que da el rope consulta por consulta.
    fn slow(rope: &Rope, byte: usize) -> Spot {
        let line = rope.byte_to_line(byte);
        Spot {
            byte,
            char: rope.byte_to_char(byte),
            point: Point::new(line, byte - rope.line_to_byte(line)),
        }
    }

    #[test]
    fn matches_the_rope_on_every_byte_of_a_text_with_many_chunks() {
        let line = "añ — línea con acentos\n\n";
        let rope = Rope::from_str(&line.repeat(400));
        assert!(rope.chunks().count() > 10);
        for byte in 0..=rope.len_bytes() {
            let boundary = rope.char_to_byte(rope.byte_to_char(byte)) == byte;
            let expected = boundary.then(|| slow(&rope, byte));
            assert_eq!(Spot::of(&rope, byte), expected, "{byte}");
            assert_eq!(is_char_boundary(&rope, byte), boundary, "{byte}");
        }
    }

    #[test]
    fn a_line_longer_than_a_chunk_starts_in_an_earlier_one() {
        let rope = Rope::from_str(&format!("a\n{}\nb", "x".repeat(10_000)));
        for byte in [2, 5_000, 10_002, 10_003] {
            assert_eq!(Spot::of(&rope, byte), Some(slow(&rope, byte)), "{byte}");
        }
    }

    #[test]
    fn outside_the_text_there_is_none() {
        let rope = Rope::from_str("ab");
        assert_eq!(Spot::of(&rope, 3), None);
        assert!(!is_char_boundary(&rope, 3));
        assert_eq!(Spot::of(&Rope::new(), 0), Some(slow(&Rope::new(), 0)));
    }
}
