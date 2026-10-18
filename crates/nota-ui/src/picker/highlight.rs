//! De las posiciones que da `nota-search` a los tramos que se resaltan.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation as _;

/// Los tramos de `text` que cubren los grafemas que empiezan en
/// `positions` (bytes, crecientes), con los vecinos unidos en uno.
#[must_use]
pub fn match_ranges(text: &str, positions: &[usize]) -> Vec<Range<usize>> {
    let mut ranges: Vec<Range<usize>> = Vec::new();
    for &start in positions {
        let Some(grapheme) = text
            .get(start..)
            .and_then(|rest| rest.graphemes(true).next())
        else {
            continue;
        };
        let end = start + grapheme.len();
        match ranges.last_mut() {
            Some(last) if last.end == start => last.end = end,
            _ => ranges.push(start..end),
        }
    }
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans(text: &str, positions: &[usize]) -> Vec<(usize, usize)> {
        match_ranges(text, positions)
            .into_iter()
            .map(|range| (range.start, range.end))
            .collect()
    }

    #[test]
    fn une_los_vecinos_y_cubre_el_grafema_entero() {
        assert_eq!(spans("ideas.md", &[0, 1, 2, 6]), [(0, 3), (6, 7)]);
        assert_eq!(spans("niño", &[2, 4]), [(2, 5)]);
        assert_eq!(spans("cafe\u{301}", &[3]), [(3, 6)]);
        assert_eq!(spans("ab", &[]), []);
        assert_eq!(spans("ab", &[7]), []);
    }
}
