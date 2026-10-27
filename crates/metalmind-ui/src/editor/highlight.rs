//! Los tramos de [`spans`](super::spans) como rectángulos sobre las líneas
//! ya pintadas: la búsqueda y la selección.

use gpui_kit::{Bounds, Hsla, PaintQuad, Pixels, fill, point, size};

use super::layout::{FrameLayout, LaidLine, LaidRow};
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
/// borde.
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
