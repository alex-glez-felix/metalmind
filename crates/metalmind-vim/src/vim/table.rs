//! `editor::format_table` y `:tableformat`.

use metalmind_buffer::{Bias, Selection};

use super::Vim;
use crate::Target;
use crate::VimError;
use crate::dot::{Change, Dot};
use crate::lines::clamp_normal;
use crate::markdown::{LazyStyles, TableFormat};

impl Vim {
    /// [`Vim::align_tables`], y `.` lo repite.
    pub(super) fn format_table(&mut self, t: &mut Target<'_>) -> Result<(), VimError> {
        self.reset();
        if Self::align_tables(t)? {
            self.last_change = Some(Dot::new(Change::FormatTable, None, None));
        } else {
            self.fail();
        }
        Ok(())
    }

    /// Alinea la tabla de cada cursor, en un solo paso de undo; cada
    /// cursor se queda en su celda. `false` si ninguno está en una tabla.
    pub(super) fn align_tables(t: &mut Target<'_>) -> Result<bool, VimError> {
        let styles = LazyStyles::new(t.doc.styles_mut());
        let mut tables: Vec<TableFormat> = Vec::new();
        let spots = t.selections.map_to(|selection| {
            let table = TableFormat::at(t.buffer, selection.head, &styles)?;
            let spot = table.spot(t.buffer, selection.head);
            let seen = tables.iter().position(|seen| seen.lines() == table.lines());
            let index = seen.unwrap_or_else(|| {
                tables.push(table);
                tables.len() - 1
            });
            Some((index, spot))
        });
        if tables.is_empty() {
            return Ok(false);
        }
        tables.sort_by_key(|table| table.lines().start);
        let edits = tables
            .iter()
            .flat_map(|table| table.edits(t.buffer))
            .collect();
        t.buffer.begin_transaction(t.selections.clone())?;
        let map = t.buffer.apply(edits)?;
        let mut spots = spots.iter();
        let after = t.selections.map(|selection| {
            let head = match spots.next().copied().flatten() {
                Some((index, spot)) => tables[index].place(t.buffer, spot),
                None => map.map(selection.head, Bias::Before),
            };
            Selection::cursor(clamp_normal(t.buffer, head))
        });
        t.buffer.commit_transaction(after.clone())?;
        *t.selections = after;
        Ok(true)
    }
}
