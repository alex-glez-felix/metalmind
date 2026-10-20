//! Las marks del margen de un panel, como marks.nvim o vim-signature: qué
//! letra se ve en cada línea.

use std::collections::BTreeMap;

use metalmind_vim::PlacedMark;

use super::{PaneId, Workspace, WorkspaceError};

/// Las marks de un documento por línea, en el orden en que se muestran: las
/// `a`–`z` antes que las `A`–`Z`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GutterMarks(BTreeMap<usize, Vec<char>>);

/// Lo que va en el margen de una línea con marks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarkLabel<'a> {
    /// La que se ve.
    pub shown: char,
    /// Las demás de la línea, en orden.
    pub others: &'a [char],
}

impl GutterMarks {
    #[must_use]
    pub fn new(marks: impl IntoIterator<Item = PlacedMark>) -> Self {
        let mut by_line: BTreeMap<usize, Vec<char>> = BTreeMap::new();
        for PlacedMark { name, line } in marks {
            by_line.entry(line).or_default().push(name);
        }
        for names in by_line.values_mut() {
            names.sort_by_key(|&name| (name.is_ascii_uppercase(), name));
        }
        Self(by_line)
    }

    #[must_use]
    pub fn at(&self, line: usize) -> Option<MarkLabel<'_>> {
        let (&shown, others) = self.0.get(&line)?.split_first()?;
        Some(MarkLabel { shown, others })
    }
}

impl MarkLabel<'_> {
    /// Hay más de una en la línea.
    #[must_use]
    pub fn more(self) -> bool {
        !self.others.is_empty()
    }

    /// El texto del tooltip, con todas.
    #[must_use]
    pub fn describe(self) -> String {
        let names: Vec<String> = std::iter::once(&self.shown)
            .chain(self.others)
            .map(char::to_string)
            .collect();
        let noun = if self.more() { "marks" } else { "mark" };
        format!("{noun} {}", names.join(", "))
    }
}

impl Workspace {
    /// Las marks del documento de `pane`, por línea; `None` si el margen
    /// no las muestra.
    ///
    /// # Errors
    /// Si `pane` no existe o su documento tampoco, lo que sería un bug.
    pub fn gutter_marks(&self, pane: PaneId) -> Result<Option<GutterMarks>, WorkspaceError> {
        if !self.gutter_marks {
            return Ok(None);
        }
        let id = self.pane(pane)?.document;
        let document = self.document(id)?;
        let file = self
            .documents
            .file(id)
            .ok_or(WorkspaceError::NoDocument(id))?;
        let placed = self
            .vim
            .placed_marks(document.state(), file, document.buffer());
        Ok(Some(GutterMarks::new(placed)))
    }
}

#[cfg(test)]
#[path = "gutter_marks_tests.rs"]
mod tests;
