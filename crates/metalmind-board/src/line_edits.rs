//! Lo que cambia entre dos versiones del archivo, en líneas enteras: lo único
//! que una edición le toca al documento (09 §4.3).

use std::ops::Range;

use similar::{Algorithm, DiffOp, capture_diff_slices};

/// Reemplazar los bytes `range` del texto viejo por `text`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineEdit<'a> {
    pub range: Range<usize>,
    pub text: &'a str,
}

/// Las ediciones que llevan de `old` a `new`, en orden y sin solaparse. Lo que
/// no cambió no se toca, ni con los mismos bytes.
#[must_use]
pub fn line_edits<'a>(old: &str, new: &'a str) -> Vec<LineEdit<'a>> {
    let old_lines: Vec<&str> = old.split_inclusive('\n').collect();
    let new_lines: Vec<&str> = new.split_inclusive('\n').collect();
    let old_starts = starts(&old_lines);
    let new_starts = starts(&new_lines);
    capture_diff_slices(Algorithm::Myers, &old_lines, &new_lines)
        .iter()
        .filter_map(|op| {
            let (old_range, new_range) = match *op {
                DiffOp::Equal { .. } => return None,
                DiffOp::Delete {
                    old_index,
                    old_len,
                    new_index,
                } => (old_index..old_index + old_len, new_index..new_index),
                DiffOp::Insert {
                    old_index,
                    new_index,
                    new_len,
                } => (old_index..old_index, new_index..new_index + new_len),
                DiffOp::Replace {
                    old_index,
                    old_len,
                    new_index,
                    new_len,
                } => (
                    old_index..old_index + old_len,
                    new_index..new_index + new_len,
                ),
            };
            Some(LineEdit {
                range: old_starts[old_range.start]..old_starts[old_range.end],
                text: &new[new_starts[new_range.start]..new_starts[new_range.end]],
            })
        })
        .collect()
}

/// Dónde empieza cada línea, y al final dónde termina la última.
fn starts(lines: &[&str]) -> Vec<usize> {
    let mut at = 0;
    let mut starts = Vec::with_capacity(lines.len() + 1);
    starts.push(0);
    for line in lines {
        at += line.len();
        starts.push(at);
    }
    starts
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

    fn apply(old: &str, edits: &[LineEdit<'_>]) -> String {
        let mut out = old.to_owned();
        for edit in edits.iter().rev() {
            out.replace_range(edit.range.clone(), edit.text);
        }
        out
    }

    #[rstest]
    #[case("a\nb\nc", "a\nB\nc", 1)]
    #[case("a\nb\nc", "a\nc", 1)]
    #[case("a\nb\nc", "a\nb\nb2\nc", 1)]
    #[case("a\nb\nc\nd", "A\nb\nc\nD", 2)]
    #[case("", "a\nb", 1)]
    #[case("a\nb", "", 1)]
    #[case("a", "a", 0)]
    #[case("uña\né\n", "uña\nÉ\n", 1)]
    fn cada_cambio_toca_solo_sus_lineas(
        #[case] old: &str,
        #[case] new: &str,
        #[case] edits: usize,
    ) {
        let found = line_edits(old, new);
        assert_eq!(found.len(), edits);
        assert_eq!(apply(old, &found), new);
    }

    #[test]
    fn la_edicion_de_una_linea_no_alcanza_a_sus_vecinas() {
        let edits = line_edits("uno\ndos\ntres\n", "uno\nDOS\ntres\n");
        assert_eq!(
            edits,
            [LineEdit {
                range: 4..8,
                text: "DOS\n"
            }]
        );
    }
}
