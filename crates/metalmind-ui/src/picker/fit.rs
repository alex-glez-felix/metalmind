//! Cómo cabe una fila del picker en una sola línea, sin GPUI: lo que se
//! recorta y dónde va la elipsis, a partir del ancho de cada grafema ya
//! medido por la vista.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation as _;

/// Lo que queda de un texto recortado alrededor de un tramo: los grafemas
/// de `kept` y si lleva elipsis a cada lado.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Around {
    pub kept: Range<usize>,
    pub ellipsis_before: bool,
    pub ellipsis_after: bool,
}

/// Lo que queda de un texto recortado por el medio: los primeros `head`
/// grafemas, una elipsis si hace falta y los grafemas desde `tail`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Middle {
    pub head: usize,
    pub tail: usize,
}

impl Middle {
    #[must_use]
    pub fn is_cut(self) -> bool {
        self.head < self.tail
    }
}

/// Recorta `widths` (el ancho de cada grafema) a `budget`, con `focus` a la
/// vista y centrado: lo que sobra se reparte a los dos lados por ancho. Si
/// `focus` solo no cabe, se queda su principio.
#[must_use]
pub fn around(widths: &[f32], focus: Range<usize>, budget: f32, ellipsis: f32) -> Around {
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
    // Sin nada que mostrar basta una elipsis.
    Around {
        ellipsis_before: kept.start > 0 && !kept.is_empty(),
        ellipsis_after: kept.end < len || (kept.is_empty() && len > 0),
        kept,
    }
}

/// Recorta `widths` a `budget` quitando del medio, con el mismo ancho a
/// cada lado; con un empate gana el final. Los últimos `keep` grafemas (el
/// nombre de una ruta) quedan enteros si caben, y entonces se reparte lo
/// que sobra entre el principio y lo de antes del nombre.
#[must_use]
pub fn middle(widths: &[f32], budget: f32, ellipsis: f32, keep: usize) -> Middle {
    let len = widths.len();
    if sum(widths) <= budget {
        return Middle {
            head: len,
            tail: len,
        };
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
    cut
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

/// Los tramos de `highlights` que caen en `kept`, contados desde el
/// principio de lo que se muestra, que empieza `offset` bytes después.
#[must_use]
pub fn cut_highlights(
    highlights: &[Range<usize>],
    kept: Range<usize>,
    offset: usize,
) -> Vec<Range<usize>> {
    let shift = |at: usize| at.clamp(kept.start, kept.end) - kept.start + offset;
    highlights
        .iter()
        .map(|range| shift(range.start)..shift(range.end))
        .filter(|range| !range.is_empty())
        .collect()
}

/// Los tramos de `highlights` que quedan en un texto de `len` bytes
/// recortado por el medio: los de los primeros `head` bytes y, corridos
/// detrás de la elipsis de `ellipsis` bytes, los desde `tail`.
#[must_use]
pub fn middle_highlights(
    highlights: &[Range<usize>],
    len: usize,
    head: usize,
    tail: usize,
    ellipsis: usize,
) -> Vec<Range<usize>> {
    let mut kept = cut_highlights(highlights, 0..head, 0);
    kept.extend(cut_highlights(highlights, tail..len, head + ellipsis));
    kept
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
/// y la `x` de cada glifo, de izquierda a derecha. Un grafema empieza en la
/// `x` del primer glifo de su byte en adelante, como `ShapedLine::x_for_index`,
/// pero en una sola pasada.
#[must_use]
pub fn grapheme_widths(starts: &[usize], glyphs: &[(usize, f32)], width: f32) -> Vec<f32> {
    let mut glyph = 0;
    let xs: Vec<f32> = starts
        .iter()
        .map(|&start| {
            while glyphs.get(glyph).is_some_and(|&(index, _)| index < start) {
                glyph += 1;
            }
            glyphs.get(glyph).map_or(width, |&(_, x)| x)
        })
        .collect();
    xs.windows(2)
        .map(|pair| (pair[1] - pair[0]).max(0.0))
        .collect()
}

fn sum(widths: &[f32]) -> f32 {
    widths.iter().sum()
}

#[cfg(test)]
#[path = "fit_tests.rs"]
mod tests;
