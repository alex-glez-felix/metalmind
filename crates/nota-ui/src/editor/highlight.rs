//! Los tramos de [`spans`](super::spans) como rectángulos sobre las líneas
//! ya pintadas.

use gpui_kit::{Bounds, Hsla, PaintQuad, Pixels, fill, point, size};

use super::layout::FrameLayout;
use super::spans::{Span, SpanEnd};

/// Medidas que no salen de la línea: el ancho de una celda, para el salto
/// de línea, y el borde derecho del panel.
#[derive(Clone, Copy)]
pub struct Cells {
    pub width: Pixels,
    pub edge: Pixels,
}

pub fn quads(
    spans: impl IntoIterator<Item = Span>,
    layout: &FrameLayout,
    cells: Cells,
    color: Hsla,
) -> Vec<PaintQuad> {
    let line_height = layout.viewport.line_height;
    spans
        .into_iter()
        .filter_map(|span| {
            let laid = layout.line(span.line)?;
            let shaped = &laid.shaped;
            let left = laid.origin.x + shaped.x_for_index(span.start);
            let right = match span.end {
                SpanEnd::At(end) => laid.origin.x + shaped.x_for_index(end),
                SpanEnd::PastText => laid.origin.x + shaped.width() + cells.width,
                SpanEnd::Edge => cells.edge,
            };
            Some(fill(
                Bounds::new(point(left, laid.origin.y), size(right - left, line_height)),
                color,
            ))
        })
        .collect()
}
