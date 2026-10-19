//! El contexto de una línea que coincide, como se pinta en su fila: sin la
//! sangría, con la primera coincidencia cerca del inicio y cortado a un
//! largo que se puede pintar aunque la línea tenga un megabyte.

use std::ops::Range;

use metalmind_search::LineMatch;
use unicode_segmentation::UnicodeSegmentation as _;

/// Grafemas que se dejan antes de la primera coincidencia.
const BEFORE: usize = 24;
/// Grafemas que se muestran como mucho.
const WIDTH: usize = 160;
const ELLIPSIS: &str = "…";

/// El texto de la fila y los bytes que se resaltan en él.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowText {
    pub text: String,
    pub highlights: Vec<Range<usize>>,
}

#[must_use]
pub fn row_text(line: &LineMatch) -> RowText {
    let first = line.matches.first().map_or(0, |found| found.start);
    let indent = line.text.len() - line.text.trim_start().len();
    let start = indent.max(start_before(&line.text[..first], BEFORE));
    let end = end_after(&line.text, start, WIDTH);
    let prefix = if start > indent { ELLIPSIS } else { "" };
    let suffix = if end < line.text.trim_end().len() {
        ELLIPSIS
    } else {
        ""
    };
    let shown = line.text[start..end].replace('\t', " ");
    let shift = |offset: usize| offset.clamp(start, end) - start + prefix.len();
    let highlights = line
        .matches
        .iter()
        .map(|found| shift(found.start)..shift(found.end))
        .filter(|range| !range.is_empty())
        .collect();
    RowText {
        text: format!("{prefix}{shown}{suffix}"),
        highlights,
    }
}

/// El byte donde empiezan los últimos `count` grafemas de `text`.
fn start_before(text: &str, count: usize) -> usize {
    text.grapheme_indices(true)
        .rev()
        .nth(count.saturating_sub(1))
        .map_or(0, |(start, _)| start)
}

/// El byte donde terminan `count` grafemas de `text` desde `start`.
fn end_after(text: &str, start: usize, count: usize) -> usize {
    text[start..]
        .grapheme_indices(true)
        .nth(count)
        .map_or(text.len(), |(offset, _)| start + offset)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    /// Las coincidencias van como `(inicio, fin)` en bytes.
    fn line(text: &str, matches: &[(usize, usize)]) -> LineMatch {
        LineMatch {
            line: 0,
            offset: 0,
            text: text.to_owned(),
            matches: matches.iter().map(|&(start, end)| start..end).collect(),
        }
    }

    fn marked(context: &RowText) -> String {
        let mut text = context.text.clone();
        for range in context.highlights.iter().rev() {
            text.insert(range.end, ']');
            text.insert(range.start, '[');
        }
        text
    }

    #[test]
    fn una_linea_corta_va_entera_sin_la_sangria() {
        let found = row_text(&line("\t  - una idea\tsuelta", &[(9, 13)]));
        assert_eq!(marked(&found), "- una [idea] suelta");
    }

    #[test]
    fn la_primera_coincidencia_queda_cerca_del_inicio() {
        let text = format!("{}idea al final", "x".repeat(100));
        let found = row_text(&line(&text, &[(100, 104)]));
        assert_eq!(
            marked(&found),
            format!("…{}[idea] al final", "x".repeat(BEFORE))
        );
    }

    #[test]
    fn una_linea_enorme_se_corta_y_lo_de_fuera_no_se_resalta() {
        let text = format!("idea {}idea", "y".repeat(1 << 20));
        let found = row_text(&line(&text, &[(0, 4), (text.len() - 4, text.len())]));
        assert_eq!(found.text.chars().count(), WIDTH + 1);
        assert!(found.text.ends_with(ELLIPSIS));
        assert_eq!(marked(&found)[..6], *"[idea]");
        assert_eq!(found.highlights.len(), 1);
    }

    #[test]
    fn corta_por_grafemas_y_no_por_bytes() {
        let text = format!("{}niño", "é".repeat(40));
        let start = text.len() - "niño".len();
        let found = row_text(&line(&text, &[(start, text.len())]));
        assert_eq!(marked(&found), format!("…{}[niño]", "é".repeat(BEFORE)));
    }
}
