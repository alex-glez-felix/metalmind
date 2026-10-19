//! Los tramos de [`spans`](super::spans) como rectángulos sobre las líneas
//! ya pintadas: la búsqueda y la selección.

use gpui_kit::{Bounds, Hsla, PaintQuad, Pixels, fill, point, size};

use super::layout::FrameLayout;
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

fn quads(
    spans: impl IntoIterator<Item = Span>,
    layout: &FrameLayout,
    cells: Cells,
    color: Hsla,
) -> Vec<PaintQuad> {
    spans
        .into_iter()
        .filter_map(|span| {
            let laid = layout.line(span.line)?;
            let left = laid.origin.x + laid.x_for_index(span.start);
            let right = match span.end {
                SpanEnd::At(end) => laid.origin.x + laid.x_for_index(end),
                SpanEnd::PastText => laid.origin.x + laid.width() + cells.width,
                SpanEnd::Edge => cells.edge,
            };
            Some(fill(
                Bounds::new(
                    point(left, laid.origin.y),
                    size(right - left, laid.text_height),
                ),
                color,
            ))
        })
        .collect()
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
    let mut painted = quads(spans(others), layout, cells, theme.search);
    painted.extend(quads(spans(current), layout, cells, theme.search_current));
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
    quads(spans, layout, cells, theme.selection)
}
