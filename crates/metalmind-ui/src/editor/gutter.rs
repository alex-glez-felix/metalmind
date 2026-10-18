//! El margen con los números de línea, a la izquierda del texto.

use gpui_kit::{Bounds, PaintQuad, Pixels, Point, ShapedLine, fill, point, px, size};

use super::element::{Typeface, shape};
use super::layout::FrameLayout;
use crate::theme::Theme;
use crate::workspace::PaneView;

/// Lo que ocupa el margen en un frame.
pub struct Gutter {
    /// Columnas, con el espacio que lo separa del texto; 0 sin números.
    pub columns: usize,
    pub width: Pixels,
}

pub struct PaintedGutter {
    pub background: PaintQuad,
    pub labels: Vec<Label>,
}

/// Un número, centrado en lo que mide su línea: más alto junto a un heading.
pub struct Label {
    pub origin: Point<Pixels>,
    pub height: Pixels,
    pub shaped: ShapedLine,
}

impl Gutter {
    /// El margen de `view` en un panel de `rows` filas, con columnas de
    /// `column_width`.
    pub fn new(view: &PaneView<'_>, rows: usize, column_width: Pixels) -> Self {
        let columns = view.numbers.width(view.buffer().line_count(), rows);
        let width = column_width * f32::from(u16::try_from(columns).unwrap_or(u16::MAX));
        Self { columns, width }
    }

    /// Lo que queda a la derecha del margen, para el texto.
    pub fn text_bounds(&self, bounds: Bounds<Pixels>) -> Bounds<Pixels> {
        let width = (bounds.size.width - self.width).max(px(0.));
        Bounds::new(
            point(bounds.left() + self.width, bounds.top()),
            size(width, bounds.size.height),
        )
    }

    /// El fondo del margen en toda la altura de `bounds`, desde su borde
    /// izquierdo, y un número por línea pintada, a la altura de su línea.
    pub fn paint(
        &self,
        view: &PaneView<'_>,
        bounds: Bounds<Pixels>,
        layout: &FrameLayout,
        typeface: &Typeface,
        theme: &Theme,
        window: &gpui_kit::Window,
    ) -> PaintedGutter {
        let text_left = layout.viewport.bounds.left();
        let left = text_left - self.width;
        let background = fill(
            Bounds::new(
                bounds.origin,
                size(text_left - bounds.left(), bounds.size.height),
            ),
            theme.gutter,
        );
        let cursor_line = view.buffer().line_of(view.cursor());
        let labels = layout
            .lines
            .iter()
            .filter_map(|laid| {
                let label = view.numbers.label(laid.line, cursor_line, self.columns)?;
                let color = if label.current {
                    theme.gutter_current
                } else {
                    theme.gutter_foreground
                };
                let shaped = shape(label.text.into(), typeface, color, None, window);
                Some(Label {
                    origin: point(left, laid.origin.y),
                    height: laid.height,
                    shaped,
                })
            })
            .collect();
        PaintedGutter { background, labels }
    }
}
