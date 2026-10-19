//! El margen a la izquierda del texto: las marks y los números de línea.

use gpui_kit::{
    AnyElement, App, AvailableSpace, Bounds, InteractiveElement as _, IntoElement, PaintQuad,
    Pixels, Point, ShapedLine, SharedString, StatefulInteractiveElement as _, Styled as _, TextRun,
    Window, div, fill, point, px, size,
};

use super::element::{Typeface, shape};
use super::layout::FrameLayout;
use crate::theme::Theme;
use crate::tooltip::Tooltip;
use crate::workspace::{GutterMarks, MarkLabel, PaneView};

/// Lo que ocupa el margen en un frame.
pub struct Gutter {
    /// Columnas, con el espacio que lo separa del texto; 0 sin números ni
    /// marks.
    pub columns: usize,
    pub width: Pixels,
    /// Las marks del documento, si el margen las muestra.
    marks: Option<GutterMarks>,
    /// Las columnas de las marks, a la izquierda de los números.
    mark_columns: usize,
    column_width: Pixels,
}

pub struct PaintedGutter {
    pub background: PaintQuad,
    pub labels: Vec<Label>,
}

/// Un número o una mark, centrado en lo que mide su línea: más alto junto a
/// un heading.
pub struct Label {
    pub origin: Point<Pixels>,
    pub height: Pixels,
    pub shaped: ShapedLine,
}

impl Gutter {
    /// El margen de `view` en un panel de `rows` filas, con columnas de
    /// `column_width`; con `marks`, les deja `mark_columns` aunque no haya
    /// ninguna, para que el ancho no salte al poner la primera.
    pub fn new(
        view: &PaneView<'_>,
        marks: Option<GutterMarks>,
        mark_columns: usize,
        rows: usize,
        column_width: Pixels,
    ) -> Self {
        let mark_columns = marks.as_ref().map_or(0, |_| mark_columns);
        let columns = view.numbers.width(view.buffer().line_count(), rows) + mark_columns;
        Self {
            columns,
            width: cells(column_width, columns),
            marks,
            mark_columns,
            column_width,
        }
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
    /// izquierdo, y en cada línea pintada su mark y su número, a su altura.
    pub fn paint(
        &self,
        view: &PaneView<'_>,
        bounds: Bounds<Pixels>,
        layout: &FrameLayout,
        typeface: &Typeface,
        theme: &Theme,
        window: &Window,
    ) -> PaintedGutter {
        let text_left = layout.viewport.bounds.left();
        let left = text_left - self.width;
        let numbers_left = left + cells(self.column_width, self.mark_columns);
        let background = fill(
            Bounds::new(
                bounds.origin,
                size(text_left - bounds.left(), bounds.size.height),
            ),
            theme.gutter,
        );
        let cursor_line = view.buffer().line_of(view.cursor());
        let number_columns = self.columns - self.mark_columns;
        let mut labels = Vec::new();
        for laid in &layout.lines {
            let at = |x: Pixels, shaped| Label {
                origin: point(x, laid.origin.y),
                height: laid.height,
                shaped,
            };
            if let Some(mark) = self.mark_at(laid.line) {
                labels.push(at(left, shape_mark(mark, typeface, theme, window)));
            }
            if let Some(label) = view.numbers.label(laid.line, cursor_line, number_columns) {
                let color = if label.current {
                    theme.gutter_current
                } else {
                    theme.gutter_foreground
                };
                let shaped = shape(label.text.into(), typeface, color, None, window);
                labels.push(at(numbers_left, shaped));
            }
        }
        PaintedGutter { background, labels }
    }

    /// Una zona por mark pintada, con el tooltip que las lista todas. Van
    /// encima del texto, así que se piden tras pintar sus líneas.
    pub fn mark_tooltips(
        &self,
        layout: &FrameLayout,
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<AnyElement> {
        let left = layout.viewport.bounds.left() - self.width;
        let width = cells(self.column_width, self.mark_columns);
        layout
            .lines
            .iter()
            .filter_map(|laid| {
                let text = SharedString::from(self.mark_at(laid.line)?.describe());
                let mut zone = div()
                    .id(("gutter-mark", laid.line))
                    .w(width)
                    .h(laid.height)
                    .tooltip(move |_, cx| Tooltip::view(text.clone(), cx))
                    .into_any_element();
                let space = size(width, laid.height).map(AvailableSpace::Definite);
                zone.prepaint_as_root(point(left, laid.origin.y), space, window, cx);
                Some(zone)
            })
            .collect()
    }

    fn mark_at(&self, line: usize) -> Option<MarkLabel<'_>> {
        self.marks.as_ref()?.at(line)
    }
}

/// La letra en el color de las marks y, si hay más en la línea, un `+`
/// discreto, del color de los números.
fn shape_mark(
    mark: MarkLabel<'_>,
    typeface: &Typeface,
    theme: &Theme,
    window: &Window,
) -> ShapedLine {
    let run = |len: usize, color| TextRun {
        len,
        font: typeface.font.clone(),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let mut text = mark.shown.to_string();
    let mut runs = vec![run(text.len(), theme.gutter_mark)];
    if mark.more() {
        text.push('+');
        runs.push(run(1, theme.gutter_foreground));
    }
    window
        .text_system()
        .shape_line(text.into(), typeface.size, &runs, None)
}

fn cells(width: Pixels, columns: usize) -> Pixels {
    width * f32::from(u16::try_from(columns).unwrap_or(u16::MAX))
}
