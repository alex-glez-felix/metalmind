//! Cómo cabe una fila del picker en una sola línea, sin GPUI: lo que se
//! recorta y dónde va la elipsis, a partir del ancho de cada grafema ya
//! medido por la vista.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation as _;

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
    let len = widths.len();
    let focus = focus.start.min(len)..focus.end.clamp(focus.start.min(len), len);
    let cost = |kept: &Range<usize>| {
        let edges = f32::from(u8::from(kept.start > 0)) + f32::from(u8::from(kept.end < len));
        sum(&widths[kept.clone()]) + edges * ellipsis
    };
    let mut kept = focus.start..focus.start;
    while kept.end < focus.end && cost(&(kept.start..kept.end + 1)) <= budget {
        kept.end += 1;
    }
    if kept.end == focus.end {
        // Un paso más, o todo lo que queda hasta el borde: llegar al borde
        // quita una elipsis, y eso puede caber aunque el paso no.
        let left =
            |kept: &Range<usize>| (kept.start > 0).then(|| [kept.start - 1..kept.end, 0..kept.end]);
        let right = |kept: &Range<usize>| {
            (kept.end < len).then(|| [kept.start..kept.end + 1, kept.start..len])
        };
        loop {
            let before = sum(&widths[kept.start..focus.start]);
            let after = sum(&widths[focus.end..kept.end]);
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
                break;
            };
            kept = next;
        }
    }
    Kept::new(len, [kept])
}

#[derive(Clone, Copy)]
struct Middle {
    head: usize,
    tail: usize,
}

/// Recorta `widths` a `budget` quitando del medio, con el mismo ancho a
/// cada lado; con un empate gana el final. Los últimos `keep` grafemas (el
/// nombre de una ruta) quedan enteros si caben, y entonces se reparte lo
/// que sobra entre el principio y lo de antes del nombre.
#[must_use]
pub fn middle(widths: &[f32], budget: f32, ellipsis: f32, keep: usize) -> Kept {
    let len = widths.len();
    if sum(widths) <= budget {
        return Kept::whole(len);
    }
    let kept_tail = len - keep.min(len);
    let tail = if sum(&widths[kept_tail..]) + ellipsis <= budget {
        kept_tail
    } else {
        len
    };
    let mut cut = Middle { head: 0, tail };
    let cost = |cut: Middle| sum(&widths[..cut.head]) + sum(&widths[cut.tail..]) + ellipsis;
    while cut.head < cut.tail {
        let front = sum(&widths[..cut.head]);
        let back = sum(&widths[cut.tail..tail]);
        let from_tail = Middle {
            tail: cut.tail - 1,
            ..cut
        };
        let from_head = Middle {
            head: cut.head + 1,
            ..cut
        };
        let order = if back <= front {
            [from_tail, from_head]
        } else {
            [from_head, from_tail]
        };
        let Some(next) = order.into_iter().find(|&next| cost(next) <= budget) else {
            break;
        };
        cut = next;
    }
    Kept::new(len, [0..cut.head, cut.tail..len])
}

/// Recorta una ruta: si `focus` (lo que coincide) cae antes del nombre, los
/// últimos `name` grafemas, se ven los dos, la carpeta recortada alrededor
/// de `focus`; si no caben juntos, gana `focus`. Sin `focus` antes del
/// nombre, se recorta por el medio con [`middle`].
#[must_use]
pub fn path(
    widths: &[f32],
    focus: Option<Range<usize>>,
    budget: f32,
    ellipsis: f32,
    name: usize,
) -> Kept {
    let len = widths.len();
    let name_start = len - name.min(len);
    let Some(focus) = focus.filter(|focus| name > 0 && focus.end <= name_start) else {
        return middle(widths, budget, ellipsis, name);
    };
    let name_width = sum(&widths[name_start..]);
    let folder = around(
        &widths[..name_start],
        focus.clone(),
        budget - name_width,
        ellipsis,
    );
    if folder.shows(&focus) {
        Kept::new(
            len,
            folder
                .ranges
                .into_iter()
                .chain(std::iter::once(name_start..len)),
        )
    } else {
        around(widths, focus, budget, ellipsis)
    }
}

/// Cuántos grafemas tiene el nombre de la ruta `text`, lo de después de la
/// última `/`; sin `/` no hay nombre que guardar.
#[must_use]
pub fn name_graphemes(text: &str) -> usize {
    text.rfind('/')
        .map_or(0, |slash| text[slash + 1..].graphemes(true).count())
}

/// Cuánto ancho le toca al texto y cuánto al detalle de la derecha en una
/// fila de `available`, con `gap` entre los dos: si no caben, el detalle se
/// queda con lo que el texto no usa, pero nunca con menos de `detail_share`
/// de la fila.
#[must_use]
pub fn split(available: f32, text: f32, detail: f32, gap: f32, detail_share: f32) -> (f32, f32) {
    if detail <= 0.0 {
        return (available.max(0.0), 0.0);
    }
    let room = (available - gap).max(0.0);
    let detail = detail.min((room - text).max(room * detail_share));
    (room - detail, detail)
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
/// siguiente a su derecha y ese avance es de su grafema, así que vale
/// también para texto de derecha a izquierda.
#[must_use]
pub fn grapheme_widths(starts: &[usize], glyphs: &[(usize, f32)], width: f32) -> Vec<f32> {
    let mut by_x = glyphs.to_vec();
    by_x.sort_by(|a, b| a.1.total_cmp(&b.1));
    let mut widths = vec![0.0; starts.len().saturating_sub(1)];
    for (at, &(index, x)) in by_x.iter().enumerate() {
        let next = by_x.get(at + 1).map_or(width, |&(_, x)| x);
        let grapheme = starts
            .partition_point(|&start| start <= index)
            .saturating_sub(1);
        if let Some(grapheme_width) = widths.get_mut(grapheme) {
            *grapheme_width += (next - x).max(0.0);
        }
    }
    widths
}

fn sum(widths: &[f32]) -> f32 {
    widths.iter().sum()
}

#[cfg(test)]
#[path = "fit_tests.rs"]
mod tests;
