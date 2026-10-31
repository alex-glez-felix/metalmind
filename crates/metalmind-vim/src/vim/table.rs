//! `editor::format_table` y `:tableformat`.

use metalmind_buffer::Selection;

use super::Vim;
use crate::Target;
use crate::VimError;
use crate::dot::{Change, Dot};
use crate::lines::clamp_normal;
use crate::markdown::{self, LazyStyles, Move};

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
        t.buffer.begin_transaction(t.selections.clone())?;
        let styles = LazyStyles::new(t.doc.styles_mut());
        let (aligned, any) = markdown::align_tables(t.buffer, t.selections, &styles, Move::Stay)?;
        let after = if any {
            aligned.map(|cursor| Selection::cursor(clamp_normal(t.buffer, cursor.head)))
        } else {
            t.selections.clone()
        };
        t.buffer.commit_transaction(after.clone())?;
        *t.selections = after;
        Ok(any)
    }
}
