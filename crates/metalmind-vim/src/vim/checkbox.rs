//! `editor::toggle_checkbox` en Normal y en Visual.

use std::collections::BTreeSet;

use metalmind_buffer::{Bias, Buffer, Selection, Selections};

use super::Vim;
use crate::dot::{Change, Dot};
use crate::lines::clamp_normal;
use crate::markdown::{LazyOutline, toggle_checkboxes};
use crate::{VimError, VisualKind};

impl Vim {
    /// En la línea de cada cursor y, con count, en las siguientes. Falla si
    /// ninguna es un ítem de lista.
    pub(super) fn toggle_checkbox(
        &mut self,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        let count = self.take_count();
        self.reset();
        let last_line = buffer.last_line();
        let lines = selections
            .iter()
            .flat_map(|selection| {
                let first = buffer.line_of(selection.head);
                let last = first.saturating_add(count.unwrap_or(1) - 1).min(last_line);
                first..=last
            })
            .collect();
        self.toggle_lines(lines, count, buffer, selections)
    }

    /// En cada línea seleccionada; el cursor queda al inicio de la
    /// selección y `.` lo repite en tantas líneas.
    pub(super) fn toggle_checkbox_visual(
        &mut self,
        kind: VisualKind,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        self.finish_visual(kind, buffer, selections);
        let lines = selections
            .iter()
            .flat_map(|s| buffer.line_of(s.start())..=buffer.line_of(s.end()))
            .collect();
        let primary = selections.primary();
        let count = buffer.line_of(primary.end()) - buffer.line_of(primary.start()) + 1;
        *selections = selections.map(|s| Selection::cursor(clamp_normal(buffer, s.start())));
        self.toggle_lines(lines, Some(count), buffer, selections)
    }

    fn toggle_lines(
        &mut self,
        lines: BTreeSet<usize>,
        count: Option<usize>,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        let edits = toggle_checkboxes(buffer, lines, &LazyOutline::new(self.layout.as_mut()));
        if edits.is_empty() {
            self.fail();
            return Ok(());
        }
        buffer.begin_transaction(selections.clone())?;
        let map = buffer.apply(edits)?;
        let after = selections
            .map(|s| Selection::cursor(clamp_normal(buffer, map.map(s.head, Bias::Before))));
        buffer.commit_transaction(after.clone())?;
        *selections = after;
        self.last_change = Some(Dot::new(Change::ToggleCheckbox, count, None));
        Ok(())
    }
}
