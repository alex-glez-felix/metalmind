//! Las medidas de un frame del editor antes de pintar: el área del texto,
//! lo que mide cada línea y la letra de las fórmulas.

use std::sync::Arc;

use gpui_kit::{Bounds, Pixels, Window, point, px, size};
use metalmind_syntax::{FormulaRows, Rows};
use metalmind_vim::LineHeights as _;

use crate::editor::gutter::cells;
use crate::editor::highlight::Cells;
use crate::editor::lines::Typeface;
use crate::math::{Geometry, Look};
use crate::theme::Theme;
use crate::workspace::PaneView;

/// Las medidas del panel en este frame, antes de pintar.
pub(super) struct Frame {
    pub(super) typeface: Typeface,
    pub(super) line_height: Pixels,
    /// El área del texto: sin el margen del tema ni el de los números, y
    /// en un panel ancho, centrada a lo ancho de lectura.
    pub(super) text: Bounds<Pixels>,
    /// La columna de la nota a lo alto del panel, con los números y el
    /// margen del tema a los lados: hasta aquí llegan los fondos.
    pub(super) column: Bounds<Pixels>,
    /// Lo que mide una columna de texto.
    pub(super) cell: Pixels,
}

impl Frame {
    /// Las medidas del panel `bounds` con un margen de números de `gutter`:
    /// el texto, a lo más del ancho de lectura del tema.
    pub(super) fn new(
        bounds: Bounds<Pixels>,
        gutter: Pixels,
        typeface: Typeface,
        line_height: Pixels,
        cell: Pixels,
        theme: &Theme,
    ) -> Self {
        let max_width = cells(cell, theme.layout.note_max_width);
        let text = reading_area(inset(bounds, theme), gutter, max_width);
        Self {
            text,
            column: column_of(bounds, text, gutter, theme.text_padding_x),
            typeface,
            line_height,
            cell,
        }
    }

    /// Las celdas del texto, con el borde de la columna como el del panel.
    pub(super) fn cells(&self) -> Cells {
        Cells {
            width: self.cell,
            edge: self.column.right(),
        }
    }

    /// Lo que se ve del texto dentro de `clip`: sin `wrap` se corre a lo
    /// ancho, así que se recorta en los números y en el borde de la columna.
    pub(super) fn text_clip(&self, clip: Bounds<Pixels>) -> Bounds<Pixels> {
        Bounds::from_corners(
            point(self.text.left(), clip.top()),
            point(self.column.right(), clip.bottom()),
        )
    }
}

/// El área del texto y los números, sin el margen del tema.
pub(super) fn inset(bounds: Bounds<Pixels>, theme: &Theme) -> Bounds<Pixels> {
    let (x, y) = (theme.text_padding_x, theme.text_padding_y);
    Bounds::new(
        point(bounds.left() + x, bounds.top() + y),
        size(
            (bounds.size.width - x * 2.).max(px(0.)),
            (bounds.size.height - y * 2.).max(px(0.)),
        ),
    )
}

/// El área del texto en `content`, con el margen de los números de
/// `gutter` a su izquierda: a lo más `max_width` de ancho, centrada si
/// sobra espacio y corrida a la derecha si a su izquierda no caben los
/// números.
pub(super) fn reading_area(
    content: Bounds<Pixels>,
    gutter: Pixels,
    max_width: Pixels,
) -> Bounds<Pixels> {
    let width = (content.size.width - gutter).max(px(0.)).min(max_width);
    let centered = content.left() + (content.size.width - width) / 2.;
    let left = centered.max(content.left() + gutter);
    Bounds::new(point(left, content.top()), size(width, content.size.height))
}

/// La columna de `text` en el panel `bounds`: con los números de `gutter`
/// y `padding` a cada lado, sin salirse del panel.
pub(super) fn column_of(
    bounds: Bounds<Pixels>,
    text: Bounds<Pixels>,
    gutter: Pixels,
    padding: Pixels,
) -> Bounds<Pixels> {
    Bounds::from_corners(
        point(
            (text.left() - gutter - padding).max(bounds.left()),
            bounds.top(),
        ),
        point(
            (text.right() + padding).min(bounds.right()),
            bounds.bottom(),
        ),
    )
}

/// Lo que mide una línea de texto, cada heading y cada fórmula, en píxeles
/// enteros: las mismas medidas para el motor, que mueve la ventana, y para
/// la vista.
pub(super) fn rows_of(line_height: Pixels, theme: &Theme, formulas: Arc<dyn FormulaRows>) -> Rows {
    let text = f32::from(line_height);
    Rows {
        text: to_units(text.round()),
        headings: theme
            .headings
            .map(|(scale, _)| to_units((text * scale).round())),
        formulas: Some(formulas),
    }
}

/// Las medidas de la letra del editor con que se componen las fórmulas.
pub(super) fn geometry_of(
    typeface: &Typeface,
    line_height: Pixels,
    theme: &Theme,
    window: &Window,
) -> Geometry {
    let text_system = window.text_system();
    let font_id = text_system.resolve_font(&typeface.font);
    Geometry {
        size: f32::from(typeface.size) * theme.text.math.0,
        line_height: f32::from(line_height),
        ascent: f32::from(text_system.ascent(font_id, typeface.size)).abs(),
        descent: f32::from(text_system.descent(font_id, typeface.size)).abs(),
        block_margin: f32::from(theme.layout.math_block_margin),
    }
}

/// Una fórmula se pinta con la letra de las fórmulas, la escala de la
/// pantalla, el color del texto y el margen del tema.
pub(super) fn look_of(geometry: Geometry, theme: &Theme, window: &Window) -> Look {
    let color = theme.ui.foreground.to_rgb();
    Look {
        size: geometry.size,
        scale: window.scale_factor(),
        color: [color.r, color.g, color.b, color.a],
        padding: f32::from(theme.layout.math_padding),
    }
}

/// Lo que la primera línea tiene oculto arriba porque es más alta que la
/// ventana y el cursor está abajo en ella, como fracción de su alto.
pub(super) fn skip_fraction(view: &PaneView<'_>, top: usize) -> f64 {
    let skip = view.viewport.skip();
    if skip == 0 {
        return 0.0;
    }
    let height = view.layout().height(top).max(1);
    f64::from(u32::try_from(skip).unwrap_or(u32::MAX))
        / f64::from(u32::try_from(height).unwrap_or(u32::MAX))
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "píxeles de un panel, ya redondeados y nunca negativos"
)]
pub(super) fn to_units(pixels: f32) -> usize {
    pixels.max(1.) as usize
}

#[cfg(test)]
mod tests {
    use gpui_kit::{Bounds, point, px, size};
    use pretty_assertions::assert_eq;

    use super::{column_of, reading_area};

    /// Un panel de `width` desde x = 100, con 10 de margen a cada lado.
    fn panel(width: f32) -> (Bounds<gpui_kit::Pixels>, Bounds<gpui_kit::Pixels>) {
        let bounds = Bounds::new(point(px(100.), px(0.)), size(px(width), px(50.)));
        let content = Bounds::new(point(px(110.), px(5.)), size(px(width - 20.), px(40.)));
        (bounds, content)
    }

    /// (ancho del panel, números, ancho de lectura) → (izquierda y ancho
    /// del texto, izquierda y derecha de la columna).
    #[test]
    fn el_texto_se_centra_a_lo_ancho_de_lectura() {
        let cases = [
            (
                "cabe justo: como sin ancho de lectura",
                (420., 40., 360.),
                (150., 360., 100., 520.),
            ),
            (
                "angosto: el texto se encoge",
                (220., 40., 360.),
                (150., 160., 100., 320.),
            ),
            (
                "ancho: centrado",
                (1020., 40., 360.),
                (430., 360., 380., 800.),
            ),
            ("sin números", (1020., 0., 360.), (430., 360., 420., 800.)),
            (
                "los números no caben a la izquierda",
                (440., 60., 360.),
                (170., 360., 100., 540.),
            ),
            (
                "ni los números caben",
                (40., 60., 360.),
                (170., 0., 100., 140.),
            ),
        ];
        for (name, (width, gutter, max), (left, text_width, column_left, column_right)) in cases {
            let (bounds, content) = panel(width);
            let text = reading_area(content, px(gutter), px(max));
            assert_eq!(
                (text.left(), text.size.width, text.top(), text.size.height),
                (px(left), px(text_width), content.top(), content.size.height),
                "{name}"
            );
            let column = column_of(bounds, text, px(gutter), px(10.));
            assert_eq!(
                (column.left(), column.right(), column.top(), column.bottom()),
                (
                    px(column_left),
                    px(column_right),
                    bounds.top(),
                    bounds.bottom()
                ),
                "{name}"
            );
        }
    }
}
