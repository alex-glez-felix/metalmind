//! `editor::toggle_checkbox` en Normal y en Visual.

use std::collections::BTreeSet;

use metalmind_buffer::{Bias, Selection};

use super::Vim;
use crate::Target;
use crate::dot::{Change, Dot};
use crate::lines::clamp_normal;
use crate::markdown::{LazyOutline, toggle_checkboxes};
use crate::{VimError, VisualKind};

impl Vim {
    /// En la línea de cada cursor y, con count, en las siguientes. Falla si
    /// ninguna es un ítem de lista.
    pub(super) fn toggle_checkbox(&mut self, t: &mut Target<'_>) -> Result<(), VimError> {
        let count = self.take_count();
        self.reset();
        let last_line = t.buffer.last_line();
        let lines = t
            .selections
            .iter()
            .flat_map(|selection| {
                let first = t.buffer.line_of(selection.head);
                let last = first.saturating_add(count.unwrap_or(1) - 1).min(last_line);
                first..=last
            })
            .collect();
        self.toggle_lines(lines, count, t)
    }

    /// En cada línea seleccionada; el cursor queda al inicio de la
    /// selección y `.` lo repite en tantas líneas.
    pub(super) fn toggle_checkbox_visual(
        &mut self,
        kind: VisualKind,
        t: &mut Target<'_>,
    ) -> Result<(), VimError> {
        self.finish_visual(kind, t);
        let lines = t
            .selections
            .iter()
            .flat_map(|s| t.buffer.line_of(s.start())..=t.buffer.line_of(s.end()))
            .collect();
        let primary = t.selections.primary();
        let count = t.buffer.line_of(primary.end()) - t.buffer.line_of(primary.start()) + 1;
        *t.selections = t
            .selections
            .map(|s| Selection::cursor(clamp_normal(t.buffer, s.start())));
        self.toggle_lines(lines, Some(count), t)
    }

    fn toggle_lines(
        &mut self,
        lines: BTreeSet<usize>,
        count: Option<usize>,
        t: &mut Target<'_>,
    ) -> Result<(), VimError> {
        let edits = toggle_checkboxes(t.buffer, lines, &LazyOutline::new(t.doc.styles_mut()));
        if edits.is_empty() {
            self.fail();
            return Ok(());
        }
        t.buffer.begin_transaction(t.selections.clone())?;
        let map = t.buffer.apply(edits)?;
        let after = t
            .selections
            .map(|s| Selection::cursor(clamp_normal(t.buffer, map.map(s.head, Bias::Before))));
        t.buffer.commit_transaction(after.clone())?;
        *t.selections = after;
        self.last_change = Some(Dot::new(Change::ToggleCheckbox, count, None));
        Ok(())
    }
}
