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
}

/// El texto ya no dice lo que la edición espera: cambió desde que se indexó.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{path} cambió: en {range:?} ya no está {expected:?}")]
pub struct StaleEdit {
    pub path: VaultPath,
    pub range: Range<usize>,
    pub expected: String,
}

impl FileEdits {
    /// Aplica las ediciones a `text` y devuelve el texto nuevo y las
    /// ediciones que lo vuelven al de antes.
    ///
    /// # Errors
    /// Si algún rango de `text` no dice lo que la edición espera; entonces
    /// no se aplica ninguna.
    pub fn apply(&self, text: &str) -> Result<(String, Self), StaleEdit> {
        let mut out = String::with_capacity(text.len());
        let mut undo = Vec::with_capacity(self.edits.len());
        let mut copied = 0;
        for edit in &self.edits {
            if edit.range.start < copied || text.get(edit.range.clone()) != Some(edit.old.as_str())
            {
                return Err(StaleEdit {
                    path: self.path.clone(),
                    range: edit.range.clone(),
                    expected: edit.old.clone(),
                });
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
        };
        Ok((out, undo))
    }
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

    #[test]
    fn a_changed_text_is_not_touched() {
        let plan = edits(vec![edit(0..1, "a", "b"), edit(2..3, "c", "d")]);
        let error = plan.apply("a x").unwrap_err();
        assert_eq!(error.range, 2..3);
        assert!(plan.apply("é").is_err());
        let overlapping = edits(vec![edit(0..2, "ab", "x"), edit(1..2, "b", "y")]);
        assert!(overlapping.apply("ab").is_err());
    }
}
