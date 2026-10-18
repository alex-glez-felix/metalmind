//! `ih` y `ah`: la sección de un heading, hasta el siguiente del mismo
//! nivel o de uno mayor.

use metalmind_buffer::Buffer;

use super::Outline;
use crate::lines::first_non_blank;

/// La primera y la última línea de la sección donde está `line`. `ah` va
/// del heading al final de la sección; `ih`, sin el heading ni las líneas en
/// blanco de los bordes. Con `count`, la sección que contiene a la de
/// `count - 1` niveles más adentro, como `2i(`. `None` si la línea no está
/// bajo ningún heading, no hay tantos niveles o `ih` queda vacío.
pub(crate) fn section(
    outline: &Outline,
    buffer: &Buffer,
    line: usize,
    count: usize,
    around: bool,
) -> Option<(usize, usize)> {
    section_in(outline, buffer, line, count, around)
}

/// En Visual por líneas, si la selección ya abarca la sección, toma la que
/// la contiene, como `vipip`.
pub(crate) fn select_section(
    outline: &Outline,
    buffer: &Buffer,
    selected: Option<(usize, usize)>,
    line: usize,
    count: usize,
    around: bool,
) -> Option<(usize, usize)> {
    // Cada count más sube un nivel: no hay más que headings.
    (count..=count + outline.headings.len())
        .map(|count| section_in(outline, buffer, line, count, around))
        .find(|found| match (found, selected) {
            (Some((first, last)), Some((from, to))) => *first < from || *last > to,
            _ => true,
        })
        .flatten()
}

fn section_in(
    outline: &Outline,
    buffer: &Buffer,
    line: usize,
    count: usize,
    around: bool,
) -> Option<(usize, usize)> {
    let headings = &outline.headings;
    let mut index = headings.iter().rposition(|heading| heading.line <= line)?;
    for _ in 1..count {
        let level = headings[index].level;
        index = headings[..index]
            .iter()
            .rposition(|heading| heading.level < level)?;
    }
    let heading = headings[index];
    let last = headings[index + 1..]
        .iter()
        .find(|next| next.level <= heading.level)
        .map_or(buffer.last_line(), |next| next.line - 1);
    if around {
        return Some((heading.line, last));
    }
    let blank = |line: usize| first_non_blank(buffer, line) == buffer.line_end(line);
    let first = (heading.line + 1..=last).find(|&line| !blank(line))?;
    let last = (first..=last).rev().find(|&line| !blank(line))?;
    Some((first, last))
}
