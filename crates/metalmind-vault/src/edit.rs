//! Ediciones por rangos exactos sobre el texto de un archivo, que se
//! comprueban antes de aplicar y se pueden deshacer.

use std::ops::Range;

use crate::path::VaultPath;

/// Cambiar `old`, en `range` del texto, por `new`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    pub range: Range<usize>,
    pub old: String,
    pub new: String,
    /// La línea del cambio, desde cero, para la vista previa.
    pub line: usize,
}

/// Las ediciones de un archivo, ordenadas y sin solaparse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEdits {
    pub path: VaultPath,
    pub edits: Vec<TextEdit>,
    /// El largo del texto con el que se planearon, si hay que exigirlo: una
    /// inserción en un archivo vacío no tiene texto que comprobar.
    pub base_len: Option<usize>,
}

/// El texto ya no dice lo que la edición espera: cambió desde que se indexó.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{path} cambió: {mismatch}")]
pub struct StaleEdit {
    pub path: VaultPath,
    pub mismatch: Mismatch,
}

/// Lo que el texto ya no cumple.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Mismatch {
    #[error("en {range:?} ya no está {expected:?}")]
    Text {
        range: Range<usize>,
        expected: String,
    },
    #[error("ya no mide {0} bytes")]
    Len(usize),
}

impl FileEdits {
    /// Aplica las ediciones a `text` y devuelve el texto nuevo y las
    /// ediciones que lo vuelven al de antes.
    ///
    /// # Errors
    /// Si algún rango de `text` no dice lo que la edición espera; entonces
    /// no se aplica ninguna.
    pub fn apply(&self, text: &str) -> Result<(String, Self), StaleEdit> {
        if let Some(len) = self.base_len.filter(|&len| len != text.len()) {
            return Err(self.stale(Mismatch::Len(len)));
        }
        let mut out = String::with_capacity(text.len());
        let mut undo = Vec::with_capacity(self.edits.len());
        let mut copied = 0;
        for edit in &self.edits {
            if edit.range.start < copied || text.get(edit.range.clone()) != Some(edit.old.as_str())
            {
                return Err(self.stale(Mismatch::Text {
                    range: edit.range.clone(),
                    expected: edit.old.clone(),
                }));
            }
            out.push_str(&text[copied..edit.range.start]);
            let start = out.len();
            out.push_str(&edit.new);
            undo.push(TextEdit {
                range: start..out.len(),
                old: edit.new.clone(),
                new: edit.old.clone(),
                line: edit.line,
            });
            copied = edit.range.end;
        }
        out.push_str(&text[copied..]);
        let undo = Self {
            path: self.path.clone(),
            edits: undo,
            base_len: self.base_len.map(|_| out.len()),
        };
        Ok((out, undo))
    }

    fn stale(&self, mismatch: Mismatch) -> StaleEdit {
        StaleEdit {
            path: self.path.clone(),
            mismatch,
        }
    }

    /// Las mismas ediciones sobre `text`, cada una con sus líneas enteras
    /// (sin el salto final), y las de una misma línea juntas: aplicarlas da
    /// lo mismo, y una vista previa puede mostrar la línea antes y después.
    ///
    /// # Errors
    /// Si algún rango de `text` no dice lo que la edición espera.
    pub fn whole_lines(&self, text: &str) -> Result<Self, StaleEdit> {
        let mut lines: Vec<TextEdit> = Vec::with_capacity(self.edits.len());
        let mut copied = 0;
        for edit in &self.edits {
            if edit.range.start < copied || text.get(edit.range.clone()) != Some(edit.old.as_str())
            {
                return Err(self.stale(Mismatch::Text {
                    range: edit.range.clone(),
                    expected: edit.old.clone(),
                }));
            }
            copied = edit.range.end;
            let start = text[..edit.range.start].rfind('\n').map_or(0, |at| at + 1);
            match lines.last_mut() {
                Some(line) if start < line.range.end || start == line.range.start => {
                    let at = edit.range.start - line.range.start + line.new.len() - line.old.len();
                    line.new.replace_range(at..at + edit.old.len(), &edit.new);
                    let end = line_end(text, edit.range.end);
                    line.new.push_str(&text[line.range.end..end]);
                    line.old.push_str(&text[line.range.end..end]);
                    line.range.end = end;
                }
                _ => {
                    let end = line_end(text, edit.range.end);
                    let old = &text[start..end];
                    let at = edit.range.start - start;
                    let mut new = old.to_owned();
                    new.replace_range(at..at + edit.old.len(), &edit.new);
                    lines.push(TextEdit {
                        range: start..end,
                        old: old.to_owned(),
                        new,
                        line: edit.line,
                    });
                }
            }
        }
        Ok(Self {
            path: self.path.clone(),
            edits: lines,
            base_len: self.base_len,
        })
    }
}

/// Dónde termina la línea de `at`, sin su salto.
fn line_end(text: &str, at: usize) -> usize {
    text[at..].find('\n').map_or(text.len(), |i| at + i)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn edit(range: Range<usize>, old: &str, new: &str) -> TextEdit {
        TextEdit {
            range,
            old: old.to_owned(),
            new: new.to_owned(),
            line: 0,
        }
    }

    fn edits(edits: Vec<TextEdit>) -> FileEdits {
        FileEdits {
            path: VaultPath::new("n.md").unwrap(),
            edits,
            base_len: None,
        }
    }

    #[test]
    fn apply_and_undo_round_trip() {
        let text = "[[a]] y [[bb]] fin";
        let plan = edits(vec![edit(2..3, "a", "xyz"), edit(10..12, "bb", "c")]);
        let (new, undo) = plan.apply(text).unwrap();
        assert_eq!(new, "[[xyz]] y [[c]] fin");
        let (back, redo) = undo.apply(&new).unwrap();
        assert_eq!(back, text);
        assert_eq!(redo, plan);
    }

    /// Con líneas enteras, las de una misma línea van juntas y aplicar da lo
    /// mismo.
    #[test]
    fn whole_lines_join_the_edits_of_a_line() {
        let text = "uno\nver [[a]] y [[a#x]]\n[[a]]\nfin";
        let plan = edits(vec![
            edit(10..11, "a", "bb"),
            edit(18..19, "a", "c"),
            edit(26..27, "a", "d"),
        ]);
        let lines = plan.whole_lines(text).unwrap();
        let shown: Vec<(&str, &str)> = lines
            .edits
            .iter()
            .map(|edit| (edit.old.as_str(), edit.new.as_str()))
            .collect();
        assert_eq!(
            shown,
            [
                ("ver [[a]] y [[a#x]]", "ver [[bb]] y [[c#x]]"),
                ("[[a]]", "[[d]]"),
            ]
        );
        assert_eq!(lines.apply(text).unwrap().0, plan.apply(text).unwrap().0);
        assert!(plan.whole_lines("otro texto que no dice eso").is_err());
        let overlapping = edits(vec![edit(0..2, "ab", "x"), edit(1..2, "b", "y")]);
        assert!(overlapping.whole_lines("ab").is_err(), "como apply");
    }

    #[test]
    fn a_changed_text_is_not_touched() {
        let plan = edits(vec![edit(0..1, "a", "b"), edit(2..3, "c", "d")]);
        let error = plan.apply("a x").unwrap_err();
        assert_eq!(
            error.mismatch,
            Mismatch::Text {
                range: 2..3,
                expected: "c".into()
            }
        );
        assert!(plan.apply("é").is_err());
        let overlapping = edits(vec![edit(0..2, "ab", "x"), edit(1..2, "b", "y")]);
        assert!(overlapping.apply("ab").is_err());
    }

    #[test]
    fn a_text_of_another_length_is_not_touched() {
        let plan = FileEdits {
            base_len: Some(0),
            ..edits(vec![edit(0..0, "", "---\n---\n")])
        };
        assert_eq!(plan.apply("x").unwrap_err().mismatch, Mismatch::Len(0));
        let (new, undo) = plan.apply("").unwrap();
        assert_eq!(undo.base_len, Some(new.len()));
        assert_eq!(undo.apply(&new).unwrap(), (String::new(), plan));
    }

    proptest::proptest! {
        /// Con ediciones al azar sobre un texto de varias líneas, las de
        /// líneas enteras dan el mismo texto.
        #[test]
        fn whole_lines_apply_the_same(
            text in "[ab\\n]{0,40}",
            cuts in proptest::collection::vec((0usize..40, 0usize..3, "[xy\\n]{0,3}"), 0..6),
        ) {
            let mut edits_at = Vec::new();
            let mut from = 0;
            for (gap, len, new) in cuts {
                let start = from + gap % 5;
                let end = start + len;
                if end > text.len() {
                    break;
                }
                let line = text[..start].matches('\n').count();
                if text[start..end].contains('\n') {
                    from = end;
                    continue;
                }
                edits_at.push(TextEdit { range: start..end, old: text[start..end].to_owned(), new, line });
                from = end + 1;
            }
            let plan = edits(edits_at);
            let lines = plan.whole_lines(&text).unwrap();
            proptest::prop_assert_eq!(lines.apply(&text).unwrap().0, plan.apply(&text).unwrap().0);
        }
    }
}
