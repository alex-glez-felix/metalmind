//! El margen a la izquierda del texto: las marks y los números de línea.

use std::rc::Rc;

use gpui_kit::{
    AnyElement, AnyTooltip, App, AvailableSpace, Bounds, Entity, InteractiveElement as _,
    IntoElement, PaintQuad, Pixels, Point, ShapedLine, Size, StatefulInteractiveElement as _,
    Styled as _, TextRun, Window, div, fill, point, px, size,
};

use super::Editor;
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

    /// Una zona por mark pintada: avisan a `editor` cuando el ratón entra o
    /// sale, para que muestre el tooltip ([`Gutter::mark_tooltip`]).
    pub fn mark_zones(
        &self,
        layout: &FrameLayout,
        editor: &Entity<Editor>,
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<AnyElement> {
        let left = layout.viewport.bounds.left() - self.width;
        let width = cells(self.column_width, self.mark_columns);
        layout
            .lines
            .iter()
            .filter(|laid| self.mark_at(laid.line).is_some())
            .map(|laid| {
                let (editor, line) = (editor.clone(), laid.line);
                let mut zone = div()
                    .id(("gutter-mark", line))
                    .w(width)
                    .h(laid.height)
                    .on_hover(move |hovered, _, cx| {
                        editor.update(cx, |editor, cx| editor.hover_mark(line, *hovered, cx));
                    })
                    .into_any_element();
                let space = size(width, laid.height).map(AvailableSpace::Definite);
                zone.prepaint_as_root(point(left, laid.origin.y), space, window, cx);
                zone
            })
            .collect()
    }

    /// El tooltip con las marks de `line`, si se ve: en la fila de abajo,
    /// desde donde empieza el texto, para no tapar la línea que describe.
    pub fn mark_tooltip(
        &self,
        line: usize,
        layout: &FrameLayout,
        clip: Bounds<Pixels>,
        theme: &Theme,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(mark) = self.mark_at(line) else {
            return;
        };
        let Some(laid) = layout.lines.iter().find(|laid| laid.line == line) else {
            return;
        };
        let view = Tooltip::view(mark.describe().into(), cx);
        let measured =
            view.clone()
                .into_any_element()
                .layout_as_root(AvailableSpace::min_size(), window, cx);
        let row = Row {
            top: laid.origin.y,
            bottom: laid.origin.y + laid.height,
            text_left: layout.viewport.bounds.left(),
        };
        let origin = tooltip_origin(row, measured, clip.bottom(), window.viewport_size().width);
        // GPUI corre el tooltip un píxel del ancla, el grosor del borde.
        let gap = theme.border_width;
        window.set_tooltip(AnyTooltip {
            view,
            mouse_position: origin - point(gap, gap),
            check_visible_and_update: Rc::new(|_, _, _| true),
        });
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

/// La fila de una línea con marks: dónde empieza y termina, y dónde empieza
/// su texto.
#[derive(Clone, Copy, Debug)]
pub struct Row {
    pub top: Pixels,
    pub bottom: Pixels,
    pub text_left: Pixels,
}

/// Dónde va un tooltip de `size` para la línea `row`: debajo de ella y
/// desde su texto, sin tapar la línea, su mark ni su número; arriba si
/// abajo pasaría de `bottom` (el texto del panel) y corrido a la izquierda
/// si pasaría de `right` (la ventana).
pub fn tooltip_origin(
    row: Row,
    size: Size<Pixels>,
    bottom: Pixels,
    right: Pixels,
) -> Point<Pixels> {
    let y = if row.bottom + size.height <= bottom {
        row.bottom
    } else {
        row.top - size.height
    };
    let x = row.text_left.min(right - size.width).max(Pixels::ZERO);
    point(x, y.max(Pixels::ZERO))
}

fn cells(width: Pixels, columns: usize) -> Pixels {
    width * f32::from(u16::try_from(columns).unwrap_or(u16::MAX))
}

#[cfg(test)]
mod tests {
    use gpui_kit::px;
    use pretty_assertions::assert_eq;

    use super::*;

    const ROW: Row = Row {
        top: px(100.),
        bottom: px(121.),
        text_left: px(350.),
    };
    const TOOLTIP: Size<Pixels> = Size {
        width: px(200.),
        height: px(30.),
    };

    #[test]
    fn va_en_la_fila_de_abajo_desde_el_texto() {
        let origin = tooltip_origin(ROW, TOOLTIP, px(700.), px(1280.));
        assert_eq!(origin, point(px(350.), px(121.)));
    }

    #[test]
    fn junto_al_borde_de_abajo_sube_a_la_fila_de_arriba() {
        let origin = tooltip_origin(ROW, TOOLTIP, px(140.), px(1280.));
        assert_eq!(origin, point(px(350.), px(70.)));
        assert!(origin.y + TOOLTIP.height <= ROW.top, "no tapa la línea");
    }

    #[test]
    fn junto_al_borde_derecho_se_corre_a_la_izquierda() {
        let origin = tooltip_origin(ROW, TOOLTIP, px(700.), px(500.));
        assert_eq!(origin, point(px(300.), px(121.)));
        let narrow = tooltip_origin(ROW, TOOLTIP, px(700.), px(150.));
        assert_eq!(narrow.x, px(0.), "nunca sale por la izquierda");
    }
}
