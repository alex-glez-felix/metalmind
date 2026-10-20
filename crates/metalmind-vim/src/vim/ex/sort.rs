//! `:sort` sobre las líneas del buffer.

use std::ops::RangeInclusive;

use metalmind_buffer::Edit;

use super::Failure;
use super::lines::{edit_lines, line_start_cursor, undo_cursor};
use crate::Target;
use crate::ex::{self, SortOptions};
use crate::search::{Case, Pattern, SearchError};
use crate::vim::Vim;

impl Vim {
    /// Como vim, el cursor queda en el primer no blanco de la primera línea.
    /// El patrón no queda como última búsqueda y, como en `ex_sort`, sigue
    /// `ignorecase` sin `smartcase`.
    pub(super) fn sort_lines(
        &mut self,
        lines: RangeInclusive<usize>,
        options: &SortOptions,
        t: &mut Target<'_>,
    ) -> Result<(), Failure> {
        let pattern = match options
            .pattern
            .as_ref()
            .map(|pattern| pattern.source.as_str())
        {
            None => None,
            Some("") => {
                let last = self
                    .search
                    .pattern
                    .as_ref()
                    .ok_or(SearchError::NoPrevious)?;
                Some(Pattern::compile(last.source(), Case::Ignore)?)
            }
            Some(source) => Some(Pattern::compile(source, Case::Ignore)?),
        };
        let by_match = options
            .pattern
            .as_ref()
            .is_some_and(|pattern| pattern.by_match);
        let (first, last) = (*lines.start(), *lines.end());
        let text = t.buffer.lines_text(first..last + 1).into_owned();
        let unsorted: Vec<&str> = text.split('\n').collect();
        let sorted = ex::sort(&unsorted, options, |line| {
            let Some(pattern) = &pattern else {
                return 0..line.len();
            };
            match pattern.line_matches(line).next() {
                Some(found) if by_match => found,
                Some(found) => found.end..line.len(),
                None => 0..0,
            }
        });
        let mut edits = Vec::new();
        if sorted != unsorted {
            let range = t.buffer.line_start(first)..t.buffer.line_end(last);
            edits.push(Edit::replace(range, sorted.join("\n")));
        }
        let cursor = t.buffer.line_of(t.selections.primary().head);
        let undo = undo_cursor(
            t.buffer,
            t.pane.viewport.columns(),
            t.selections,
            lines.contains(&cursor),
            first,
        );
        Ok(edit_lines(t.buffer, t.selections, undo, edits, |buffer| {
            line_start_cursor(buffer, first)
        })?)
    }
}
