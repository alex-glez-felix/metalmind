//! Cómo cabe una fila del picker en una sola línea, sin GPUI: lo que se
//! recorta y dónde va la elipsis, a partir del ancho de cada grafema ya
//! medido por la vista.

mod path;
mod widths;

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation as _;

pub use self::path::path;
use self::widths::Widths;

/// Lo que se pinta en lugar de lo que no cabe.
pub const ELLIPSIS: &str = "…";

/// Lo que queda de un texto de `len` grafemas que no cabe: los tramos que
/// se ven, en orden y sin tocarse. Cada hueco (al principio, entre dos
/// tramos o al final) se pinta como una elipsis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Kept {
    pub ranges: Vec<Range<usize>>,
    pub len: usize,
}

impl Kept {
    fn new(len: usize, ranges: impl IntoIterator<Item = Range<usize>>) -> Self {
        let mut kept: Vec<Range<usize>> = Vec::new();
        for range in ranges.into_iter().filter(|range| !range.is_empty()) {
            match kept.last_mut() {
                Some(last) if last.end == range.start => last.end = range.end,
                _ => kept.push(range),
            }
        }
        Self { ranges: kept, len }
    }

    fn whole(len: usize) -> Self {
        Self::new(len, std::iter::once(0..len))
    }

    /// Si se ve entero `range`.
    fn shows(&self, range: &Range<usize>) -> bool {
        self.ranges
            .iter()
            .any(|kept| kept.start <= range.start && range.end <= kept.end)
    }
}

/// Recorta `widths` (el ancho de cada grafema) a `budget`, con `focus` a la
/// vista y centrado: lo que sobra se reparte a los dos lados por ancho. Si
/// `focus` solo no cabe, se queda su principio.
#[must_use]
pub fn around(widths: &[f32], focus: Range<usize>, budget: f32, ellipsis: f32) -> Kept {
    around_all(&Widths::new(widths), focus, budget, ellipsis)
}

fn around_all(widths: &Widths, focus: Range<usize>, budget: f32, ellipsis: f32) -> Kept {
    let len = widths.len();
    let kept = around_within(widths, 0..len, &(0..len), focus, budget, ellipsis);
    Kept::new(len, [kept])
}

/// Como [`around`], pero sin salir de `bounds`, y con una elipsis por cada
/// lado en el que lo que se queda no llega a `gaps` (lo que ya se ve).
fn around_within(
    widths: &Widths,
    bounds: Range<usize>,
    gaps: &Range<usize>,
    focus: Range<usize>,
    budget: f32,
    ellipsis: f32,
) -> Range<usize> {
    let start = focus.start.clamp(bounds.start, bounds.end);
    let focus = start..focus.end.clamp(start, bounds.end);
    let cost = |kept: &Range<usize>| {
        let edges =
            f32::from(u8::from(kept.start > gaps.start)) + f32::from(u8::from(kept.end < gaps.end));
        widths.sum(kept.clone()) + edges * ellipsis
    };
    // Si `focus` no cabe por sus elipsis, puede caber estirado hasta un
    // borde, donde no la paga; si tampoco, se queda su principio.
    let mut kept = focus.clone();
    if !kept.is_empty() && cost(&kept) > budget {
        let reaching = [
            focus.start..bounds.end,
            bounds.start..focus.end,
            bounds.clone(),
        ];
        if let Some(fits) = reaching.into_iter().find(|range| cost(range) <= budget) {
            kept = fits;
        } else {
            kept.end = kept.start;
            while kept.end < focus.end && cost(&(kept.start..kept.end + 1)) <= budget {
                kept.end += 1;
            }
            return kept;
        }
    }
    // Un paso más, o todo lo que queda hasta el borde: llegar al borde
    // quita una elipsis, y eso puede caber aunque el paso no.
    let left = |kept: &Range<usize>| {
        (kept.start > bounds.start).then(|| [kept.start - 1..kept.end, bounds.start..kept.end])
    };
    let right = |kept: &Range<usize>| {
        (kept.end < bounds.end).then(|| [kept.start..kept.end + 1, kept.start..bounds.end])
    };
    loop {
        let before = widths.sum(kept.start..focus.start);
        let after = widths.sum(focus.end..kept.end);
        let sides = if before <= after {
            [left(&kept), right(&kept)]
        } else {
            [right(&kept), left(&kept)]
        };
        let Some(next) = sides
            .into_iter()
            .flatten()
            .flatten()
            .find(|range| cost(range) <= budget)
        else {
            return kept;
        };
        kept = next;
    }
}

#[derive(Clone, Copy)]
struct Middle {
    head: usize,
    tail: usize,
}

/// Recorta `widths` a `budget` quitando del medio, con el mismo ancho a
/// cada lado; con un empate gana el final.
fn middle_all(widths: &Widths, budget: f32, ellipsis: f32) -> Kept {
    let len = widths.len();
    Kept::new(len, middle_within(widths, 0..len, budget, ellipsis))
}

/// Como [`middle_all`], pero solo con los grafemas de `bounds`.
fn middle_within(
    widths: &Widths,
    bounds: Range<usize>,
    budget: f32,
    ellipsis: f32,
) -> [Range<usize>; 2] {
    if widths.sum(bounds.clone()) <= budget {
        return [bounds.clone(), bounds.end..bounds.end];
    }
    let mut cut = Middle {
        head: bounds.start,
        tail: bounds.end,
    };
    let front = |cut: Middle| widths.sum(bounds.start..cut.head);
    let back = |cut: Middle| widths.sum(cut.tail..bounds.end);
    let cost = |cut: Middle| front(cut) + back(cut) + ellipsis;
    while cut.head < cut.tail {
        let from_tail = Middle {
            tail: cut.tail - 1,
            ..cut
        };
        let from_head = Middle {
            head: cut.head + 1,
            ..cut
        };
        let order = if back(cut) <= front(cut) {
            [from_tail, from_head]
        } else {
            [from_head, from_tail]
        };
        let Some(next) = order.into_iter().find(|&next| cost(next) <= budget) else {
            break;
        };
        cut = next;
    }
    [bounds.start..cut.head, cut.tail..bounds.end]
}

/// Dónde están las `/` de `text`, en grafemas.
#[must_use]
pub fn slashes(text: &str) -> Vec<usize> {
    text.graphemes(true)
        .enumerate()
        .filter(|&(_, grapheme)| grapheme == "/")
        .map(|(at, _)| at)
        .collect()
}

/// Cuánto ancho le toca al texto y cuánto al detalle de la derecha en una
/// fila de `available`, con `gap` entre los dos: si no caben, el detalle se
/// queda con lo que el texto no usa, pero nunca con menos de `detail_share`
/// de la fila. Sin lugar ni para una elipsis, el detalle no se pinta
/// (`None`) y el texto usa toda la fila.
#[must_use]
pub fn split(
    available: f32,
    text: f32,
    detail: Option<f32>,
    gap: f32,
    detail_share: f32,
    ellipsis: f32,
) -> (f32, Option<f32>) {
    let room = (available - gap).max(0.0);
    let detail = detail
        .map(|detail| detail.min((room - text).max(room * detail_share)))
        .filter(|&detail| detail >= ellipsis);
    match detail {
        Some(detail) => (room - detail, Some(detail)),
        None => (available.max(0.0), None),
    }
}

/// El alto de la lista: filas enteras, todas si caben y si no `max_rows`
/// (con la lista vacía, una para decirlo), más el borde de arriba, que
/// cuenta dentro del alto.
#[must_use]
pub fn list_height(len: usize, max_rows: u8, row_height: f32, border: f32) -> f32 {
    let rows = u8::try_from(len)
        .unwrap_or(u8::MAX)
        .clamp(1, max_rows.max(1));
    f32::from(rows) * row_height + border
}

/// El texto que se pinta para `kept` y sus resaltados: `starts` son los
/// bytes donde empieza cada grafema de `text`, más el final, y
/// `highlights` los bytes que coinciden.
#[must_use]
pub fn render(
    text: &str,
    starts: &[usize],
    kept: &Kept,
    highlights: &[Range<usize>],
) -> (String, Vec<Range<usize>>) {
    let byte = |grapheme: usize| starts.get(grapheme).copied().unwrap_or(text.len());
    let mut shown = String::new();
    let mut shown_highlights = Vec::new();
    let mut end = 0;
    for range in &kept.ranges {
        if range.start > end {
            shown.push_str(ELLIPSIS);
        }
        let bytes = byte(range.start)..byte(range.end);
        shown_highlights.extend(cut_highlights(highlights, bytes.clone(), shown.len()));
        shown.push_str(&text[bytes]);
        end = range.end;
    }
    if end < kept.len {
        shown.push_str(ELLIPSIS);
    }
    (shown, shown_highlights)
}

/// Los tramos de `highlights` que caen en `kept`, contados desde `offset`.
fn cut_highlights(
    highlights: &[Range<usize>],
    kept: Range<usize>,
    offset: usize,
) -> impl Iterator<Item = Range<usize>> {
    let shift = move |at: usize| at.clamp(kept.start, kept.end) - kept.start + offset;
    highlights
        .iter()
        .map(move |range| shift(range.start)..shift(range.end))
        .filter(|range| !range.is_empty())
}

/// Un salto de línea o un tab en el texto de una fila (un nombre de archivo
/// puede tenerlos) se pinta como espacio: mide un byte igual, así que los
/// resaltados no se mueven.
#[must_use]
pub fn one_line(text: &str) -> String {
    text.replace(['\n', '\r', '\t'], " ")
}

/// El ancho de cada grafema de una línea ya con forma: `starts` son los
/// bytes donde empieza cada uno, más el final del texto, y `glyphs` el byte
/// y la `x` de cada glifo, en cualquier orden. Cada glifo avanza hasta el
/// siguiente a su derecha, así que vale también de derecha a izquierda, y
/// ese avance se reparte entre los grafemas de su clúster, desde su byte
/// hasta el del glifo siguiente en el texto: una ligadura mide lo que
/// ocupan todos los que junta.
#[must_use]
pub fn grapheme_widths(starts: &[usize], glyphs: &[(usize, f32)], width: f32) -> Vec<f32> {
    let end = starts.last().copied().unwrap_or(0);
    let mut by_x = glyphs.to_vec();
    by_x.sort_by(|a, b| a.1.total_cmp(&b.1));
    let mut by_index: Vec<usize> = glyphs.iter().map(|&(index, _)| index).collect();
    by_index.sort_unstable();
    let grapheme_of = |byte: usize| {
        starts
            .partition_point(|&start| start <= byte)
            .saturating_sub(1)
    };
    let mut widths = vec![0.0; starts.len().saturating_sub(1)];
    for (at, &(index, x)) in by_x.iter().enumerate() {
        let advance = (by_x.get(at + 1).map_or(width, |&(_, x)| x) - x).max(0.0);
        let cluster_end = by_index
            .get(by_index.partition_point(|&other| other <= index))
            .copied()
            .unwrap_or(end);
        let first = grapheme_of(index);
        let last = grapheme_of(cluster_end.saturating_sub(1)).max(first);
        let Some(covered) = widths.get_mut(first..=last) else {
            continue;
        };
        let share = advance / f32::from(u16::try_from(covered.len()).unwrap_or(u16::MAX));
        for grapheme_width in covered {
            *grapheme_width += share;
        }
    }
    widths
}

#[cfg(test)]
mod tests;
