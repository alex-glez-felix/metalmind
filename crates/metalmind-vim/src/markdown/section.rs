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
    let index = ancestors(outline, line).nth(count.saturating_sub(1))?;
    bounds(outline, buffer, index, around)
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
    ancestors(outline, line)
        .skip(count.saturating_sub(1))
        .map(|index| bounds(outline, buffer, index, around))
        .find(|found| match (found, selected) {
            (Some((first, last)), Some((from, to))) => *first < from || *last > to,
            _ => true,
        })
        .flatten()
}

/// El heading de la sección de `line` y los que la contienen, de adentro
/// hacia afuera: a lo sumo uno por nivel.
fn ancestors(outline: &Outline, line: usize) -> impl Iterator<Item = usize> + '_ {
    let headings = &outline.headings;
    let first = headings.iter().rposition(|heading| heading.line <= line);
    std::iter::successors(first, move |&index| {
        let level = headings[index].level;
        headings[..index]
            .iter()
            .rposition(|heading| heading.level < level)
    })
}

fn bounds(
    outline: &Outline,
    buffer: &Buffer,
    index: usize,
    around: bool,
) -> Option<(usize, usize)> {
    let headings = &outline.headings;
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
