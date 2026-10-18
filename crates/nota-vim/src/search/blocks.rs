//! El buffer en bloques de líneas enteras, para que el regex corra sobre
//! texto seguido en vez de arrancar una búsqueda por línea.

use std::ops::Range;

use nota_buffer::Buffer;

/// Lo que mide un bloque, más lo que falte para cerrar su última línea.
const BLOCK_BYTES: usize = 64 * 1024;

/// `lines` en bloques consecutivos, de arriba abajo.
pub(super) fn blocks(buffer: &Buffer, lines: Range<usize>) -> impl Iterator<Item = Range<usize>> {
    let end = lines.end.min(buffer.line_count());
    let mut first = lines.start;
    std::iter::from_fn(move || {
        if first >= end {
            return None;
        }
        let reach = buffer.line_of(buffer.line_start(first) + BLOCK_BYTES);
        let block = first..(reach + 1).min(end);
        first = block.end;
        Some(block)
    })
}

/// `lines` en bloques consecutivos, de abajo arriba.
pub(super) fn blocks_rev(
    buffer: &Buffer,
    lines: Range<usize>,
) -> impl Iterator<Item = Range<usize>> {
    let mut end = lines.end.min(buffer.line_count());
    std::iter::from_fn(move || {
        if end <= lines.start {
            return None;
        }
        let reach = buffer.line_of(buffer.line_end(end - 1).saturating_sub(BLOCK_BYTES));
        let block = reach.max(lines.start)..end;
        end = block.start;
        Some(block)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1 000 líneas de 200 bytes: unos 200 KiB, varios bloques.
    fn note() -> Buffer {
        Buffer::from(format!("{}\n", "x".repeat(199)).repeat(1000).as_str())
    }

    #[test]
    fn blocks_cover_each_line_once_in_order() {
        let buffer = note();
        let down: Vec<_> = blocks(&buffer, 3..990).collect();
        assert!(down.len() > 2, "{down:?}");
        assert_eq!(down.first().map(|block| block.start), Some(3));
        assert_eq!(down.last().map(|block| block.end), Some(990));
        assert!(down.windows(2).all(|pair| pair[0].end == pair[1].start));
        let mut up: Vec<_> = blocks_rev(&buffer, 3..990).collect();
        up.reverse();
        assert_eq!(up.first().map(|block| block.start), Some(3));
        assert_eq!(up.last().map(|block| block.end), Some(990));
        assert!(up.windows(2).all(|pair| pair[0].end == pair[1].start));
    }

    #[test]
    fn a_block_stops_after_the_line_that_reaches_its_size() {
        let buffer = note();
        for block in blocks(&buffer, 0..1001).chain(blocks_rev(&buffer, 0..1001)) {
            assert!(
                buffer.lines_text(block.clone()).len() <= BLOCK_BYTES + 200,
                "{block:?}"
            );
        }
    }

    #[test]
    fn a_line_longer_than_a_block_is_a_block() {
        let long = "x".repeat(3 * BLOCK_BYTES);
        let buffer = Buffer::from(format!("a\n{long}\nb").as_str());
        assert_eq!(blocks(&buffer, 1..3).collect::<Vec<_>>(), [1..2, 2..3]);
        assert_eq!(blocks_rev(&buffer, 0..2).collect::<Vec<_>>(), [1..2, 0..1]);
    }

    #[test]
    fn ranges_past_the_end_stop_at_the_last_line() {
        let buffer = Buffer::from("a\nb");
        assert_eq!(blocks(&buffer, 0..9).collect::<Vec<_>>(), vec![0..2]);
        assert_eq!(blocks_rev(&buffer, 0..9).collect::<Vec<_>>(), vec![0..2]);
        assert_eq!(blocks(&buffer, 2..2).count(), 0);
    }
}
