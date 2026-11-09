//! Los tramos de [`spans`](super::spans) como rectángulos sobre las líneas
//! ya pintadas: la búsqueda y la selección.

use gpui_kit::{Bounds, Hsla, PaintQuad, Pixels, fill, point, size};

use super::layout::{FrameLayout, LaidLine, LaidRow, Piece};
use super::spans::{Match, Span, SpanEnd, search_spans, selection_spans};
use crate::theme::Theme;
use crate::workspace::PaneView;

/// Medidas que no salen de la línea: el ancho de una celda, para el salto
/// de línea, y el borde derecho del panel.
#[derive(Clone, Copy)]
pub struct Cells {
    pub width: Pixels,
    pub edge: Pixels,
}

/// Un rectángulo por fila de cada tramo: el que cruza filas de una línea
/// partida llega al final de cada una, salvo la última.
fn quads(
    spans: impl IntoIterator<Item = Span>,
    layout: &FrameLayout,
    cells: Cells,
    color: Hsla,
) -> Vec<PaintQuad> {
    let mut painted = Vec::new();
    for span in spans {
        let Some(laid) = layout.line(span.line) else {
            continue;
        };
        for (row, end) in rows_of(laid, &span) {
            let origin = laid.row_origin(row);
            let left = origin.x + row.x_for_index(span.start.max(row.source.start));
            let right = match end {
                Some(SpanEnd::At(end)) => origin.x + row.x_for_end(end),
                Some(SpanEnd::PastText) => origin.x + row.width() + cells.width,
                Some(SpanEnd::Edge) => cells.edge,
                None => origin.x + row.width(),
            };
            painted.push(fill(
                Bounds::new(point(left, origin.y), size(right - left, row.height)),
                color,
            ));
        }
    }
    painted
}

/// Las filas de `laid` que toca `span`, con su final en la última; las de
/// antes llegan al final de su texto o, si el tramo llega al borde, al
/// borde. Lo que toca una fórmula partida en filas la cubre en todas.
fn rows_of<'a>(
    laid: &'a LaidLine,
    span: &Span,
) -> impl Iterator<Item = (&'a LaidRow, Option<SpanEnd>)> + 'a {
    let first = laid.row_index(span.start);
    let last = match span.end {
        SpanEnd::At(end) if end > span.start => laid.row_index(end - 1),
        SpanEnd::At(_) => first,
        SpanEnd::PastText | SpanEnd::Edge => laid.rows.len().saturating_sub(1),
    };
    let (first, last) = with_formula_rows(laid, span, first, last);
    let end = span.end;
    laid.rows
        .iter()
        .enumerate()
        .skip(first)
        .take(last.saturating_sub(first) + 1)
        .map(move |(index, row)| {
            let edge = matches!(end, SpanEnd::Edge).then_some(SpanEnd::Edge);
            (row, if index == last { Some(end) } else { edge })
        })
}

/// `first..=last` ampliadas a las filas de las fórmulas que toca `span`: la
/// imagen de una fórmula partida va sobre todas sus filas.
fn with_formula_rows(laid: &LaidLine, span: &Span, first: usize, last: usize) -> (usize, usize) {
    let end = match span.end {
        SpanEnd::At(end) => end,
        SpanEnd::PastText | SpanEnd::Edge => laid.len(),
    };
    let touches = |piece: &Piece| {
        piece.text.is_none() && piece.source.start < end && span.start < piece.source.end
    };
    let holds = |row: &LaidRow| row.pieces.iter().any(touches);
    let from = laid.rows.iter().position(holds).unwrap_or(first);
    let to = laid.rows.iter().rposition(holds).unwrap_or(last);
    (first.min(from), last.max(to))
}

pub fn search_quads(
    view: &PaneView<'_>,
    layout: &FrameLayout,
    cells: Cells,
    theme: &Theme,
) -> Vec<PaintQuad> {
    let Some(pattern) = view.search else {
        return Vec::new();
    };
    let found = search_spans(
        pattern,
        view.buffer(),
        layout.visible_lines(),
        view.cursor(),
    );
    let (current, others): (Vec<_>, Vec<_>) = found.into_iter().partition(|found| found.current);
    let spans = |matches: Vec<Match>| matches.into_iter().map(|found| found.span);
    let mut painted = quads(spans(others), layout, cells, theme.ui.search);
    painted.extend(quads(
        spans(current),
        layout,
        cells,
        theme.ui.search_current,
    ));
    painted
}

pub fn selection_quads(
    view: &PaneView<'_>,
    layout: &FrameLayout,
    cells: Cells,
    theme: &Theme,
) -> Vec<PaintQuad> {
    let visible = layout.visible_lines();
    let spans = selection_spans(
        view.buffer(),
        view.selection,
        view.mode,
        &view.viewport,
        visible,
    );
    quads(spans, layout, cells, theme.ui.selection)
}

#[cfg(test)]
mod tests {
    use gpui_kit::{point, px};
    use pretty_assertions::assert_eq;

    use super::*;

    fn row(source: std::ops::Range<usize>, top: f32, pieces: Vec<Piece>) -> LaidRow {
        LaidRow {
            source,
            top: px(top),
            height: px(20.),
            pieces,
        }
    }

    fn formula(source: std::ops::Range<usize>) -> Piece {
        Piece {
            source,
            x: px(10.),
            width: px(50.),
            text: None,
        }
    }

    fn line(rows: Vec<LaidRow>) -> LaidLine {
        LaidLine {
            line: 0,
            origin: point(px(0.), px(0.)),
            height: rows.last().map_or(px(0.), |row| row.top + row.height),
            font_size: px(15.),
            rows,
            formulas: Vec::new(),
            errors: Vec::new(),
            code_block: false,
        }
    }

    fn rows(laid: &LaidLine, start: usize, end: usize) -> Vec<f32> {
        rows_until(laid, start, SpanEnd::At(end))
    }

    fn rows_until(laid: &LaidLine, start: usize, end: SpanEnd) -> Vec<f32> {
        let span = Span {
            line: 0,
            start,
            end,
        };
        rows_of(laid, &span)
            .map(|(row, _)| f32::from(row.top))
            .collect()
    }

    /// Una coincidencia en la última fila de una fórmula partida la cubre en
    /// todas sus filas, porque su imagen va sobre todas.
    #[test]
    fn lo_que_toca_una_formula_partida_la_cubre_en_todas_sus_filas() {
        let laid = line(vec![
            row(0..10, 0., vec![formula(0..30)]),
            row(10..20, 20., vec![formula(0..30)]),
            row(20..30, 40., vec![formula(0..30)]),
        ]);
        assert_eq!(rows(&laid, 22, 25), [0., 20., 40.]);
    }

    #[test]
    fn una_formula_de_una_fila_no_amplia_el_tramo() {
        let laid = line(vec![
            row(0..10, 0., vec![formula(2..8)]),
            row(10..20, 20., Vec::new()),
        ]);
        assert_eq!(rows(&laid, 12, 15), [20.]);
        assert_eq!(rows(&laid, 3, 5), [0.]);
    }

    /// `v` desde el fin de una línea que es una fórmula partida selecciona
    /// solo el salto: la fórmula no se toca y queda sin resaltar, como en vim.
    #[test]
    fn el_salto_tras_una_formula_partida_no_la_cubre() {
        let laid = line(vec![
            row(0..10, 0., vec![formula(0..30)]),
            row(10..20, 20., vec![formula(0..30)]),
            row(20..30, 40., vec![formula(0..30)]),
        ]);
        assert_eq!(rows_until(&laid, 30, SpanEnd::PastText), [40.]);
        assert_eq!(rows_until(&laid, 29, SpanEnd::PastText), [0., 20., 40.]);
    }

    #[test]
    fn otra_formula_en_la_fila_de_una_partida_no_la_cubre() {
        let laid = line(vec![
            row(0..10, 0., vec![formula(0..15)]),
            row(10..20, 20., vec![formula(0..15), formula(15..20)]),
        ]);
        assert_eq!(rows(&laid, 15, 18), [20.]);
        assert_eq!(rows(&laid, 12, 13), [0., 20.]);
    }
}
