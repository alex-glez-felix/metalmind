//! `]]` y `[[`: como en nvim con `ft=markdown`, van al inicio de la línea
//! del heading siguiente o anterior, sin contar la del cursor.

use super::Outline;

/// La línea del `count`-ésimo heading tras `line`, o la del último si hay
/// menos; `None` si no hay ninguno.
pub(crate) fn next_heading(outline: &Outline, line: usize, count: usize) -> Option<usize> {
    outline
        .headings
        .iter()
        .filter(|heading| heading.line > line)
        .take(count)
        .last()
        .map(|heading| heading.line)
}

/// Como [`next_heading`], hacia arriba.
pub(crate) fn previous_heading(outline: &Outline, line: usize, count: usize) -> Option<usize> {
    outline
        .headings
        .iter()
        .rev()
        .filter(|heading| heading.line < line)
        .take(count)
        .last()
        .map(|heading| heading.line)
}
