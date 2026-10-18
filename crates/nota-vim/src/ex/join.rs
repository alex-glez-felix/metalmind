//! Unir líneas como `do_join` de vim (`:j`, y mañana `J`).

use crate::lines::is_blank;

/// Une `lines` en una. Con `spaces` (`:j`), a cada línea siguiente se le
/// quita la sangría y va tras un espacio, salvo que quede vacía, empiece
/// con `)`, lo anterior esté vacío o termine en tab, o termine en espacio,
/// que ya separa. Sin `spaces` (`:j!`) se pegan tal cual. Como nvim con
/// `nojoinspaces`; a diferencia de su `formatoptions` por default, no quita
/// los marcadores de comentario.
pub(crate) fn join<'a>(lines: impl IntoIterator<Item = &'a str>, spaces: bool) -> String {
    let mut joined = String::new();
    // Los dos últimos caracteres de la línea anterior.
    let mut end: (Option<char>, Option<char>) = (None, None);
    for (index, line) in lines.into_iter().enumerate() {
        let mut current = line;
        if spaces && index > 0 {
            current = current.trim_start_matches(is_blank);
            let separates = !current.is_empty()
                && !current.starts_with(')')
                && !joined.is_empty()
                && end.0 != Some('\t');
            if separates && end.0 != Some(' ') {
                joined.push(' ');
            }
        }
        joined.push_str(current);
        let mut tail = current.chars().rev();
        end = (tail.next(), tail.next());
    }
    joined
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_like_vim() {
        let cases: [(&[&str], bool, &str); 8] = [
            (&["ab", "   cd"], true, "ab cd"),
            (&["ab", "   cd"], false, "ab   cd"),
            (&["ab", "", "x"], true, "ab x"),
            (&["ab ", "cd"], true, "ab cd"),
            (&["ab", ")x"], true, "ab)x"),
            (&["ab\t", "cd"], true, "ab\tcd"),
            (&["", "  cd"], true, "cd"),
            (&["ab.", "cd"], true, "ab. cd"),
        ];
        for (lines, spaces, expected) in cases {
            assert_eq!(join(lines.iter().copied(), spaces), expected, "{lines:?}");
        }
    }
}
