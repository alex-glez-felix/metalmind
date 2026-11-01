//! Lo que cambió entre dos versiones de un archivo, en líneas enteras: así
//! se muestra en una vista previa y se aplica junto con otros cambios del
//! mismo texto.

use std::ops::Range;

/// Cambiar las líneas `range` del texto, que dicen `old`, por `new`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineEdit {
    /// Del principio de la primera línea que cambia al final de la última,
    /// con su salto.
    pub range: Range<usize>,
    pub old: String,
    pub new: String,
    /// La primera que cambia, desde cero.
    pub line: usize,
}

impl LineEdit {
    /// Las líneas de `before` que cambian para llegar a `after`; `None` si
    /// son iguales.
    #[must_use]
    pub fn between(before: &str, after: &str) -> Option<Self> {
        if before == after {
            return None;
        }
        let (old, new) = (lines(before), lines(after));
        let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
        let suffix = old[prefix..]
            .iter()
            .rev()
            .zip(new[prefix..].iter().rev())
            .take_while(|(a, b)| a == b)
            .count();
        let start: usize = old[..prefix].iter().map(|line| line.len()).sum();
        let end = |text: &str, lines: &[&str]| -> usize {
            text.len()
                - lines[lines.len() - suffix..]
                    .iter()
                    .map(|line| line.len())
                    .sum::<usize>()
        };
        let (old_end, new_end) = (end(before, &old), end(after, &new));
        Some(Self {
            range: start..old_end,
            old: before[start..old_end].to_owned(),
            new: after[start..new_end].to_owned(),
            line: prefix,
        })
    }
}

fn lines(text: &str) -> Vec<&str> {
    text.split_inclusive('\n').collect()
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn edit(before: &str, after: &str) -> (String, String, usize) {
        let edit = LineEdit::between(before, after).unwrap();
        assert_eq!(&before[edit.range.clone()], edit.old);
        let mut applied = before.to_owned();
        applied.replace_range(edit.range, &edit.new);
        assert_eq!(applied, after);
        (edit.old, edit.new, edit.line)
    }

    #[test]
    fn toma_las_lineas_enteras_que_cambian() {
        let before = "views:\n  gs: \"#gs -#x\"\n  hoy: \"#hoy\"\n";
        let after = "views:\n  gs: \"#estudio -#x\"\n  hoy: \"#hoy\"\n";
        assert_eq!(
            edit(before, after),
            (
                "  gs: \"#gs -#x\"\n".to_owned(),
                "  gs: \"#estudio -#x\"\n".to_owned(),
                1
            )
        );
    }

    #[test]
    fn al_principio_al_final_y_con_lineas_de_mas() {
        assert_eq!(edit("a: 1\nb: 2", "a: 3\nb: 2").2, 0);
        assert_eq!(edit("a: 1\nb: 2", "a: 1\nb: 4").2, 1);
        assert_eq!(
            edit("a:\n  - x\n  - y\nb: 2\n", "a: \"z\"\nb: 2\n"),
            ("a:\n  - x\n  - y\n".to_owned(), "a: \"z\"\n".to_owned(), 0)
        );
        assert_eq!(
            edit("a: 1\n", "a: 1\nb: 2\n"),
            (String::new(), "b: 2\n".to_owned(), 1)
        );
        assert_eq!(LineEdit::between("a: 1\n", "a: 1\n"), None);
    }
}
