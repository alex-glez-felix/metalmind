//! Las medidas de un frame del editor antes de pintar: el área del texto,
//! lo que mide cada línea y la letra de las fórmulas.

use std::sync::Arc;

use gpui_kit::{Bounds, Pixels, Window, point, px, size};
use metalmind_syntax::{FormulaRows, Rows};
use metalmind_vim::LineHeights as _;

use crate::editor::lines::Typeface;
use crate::math::{Geometry, Look};
use crate::theme::Theme;
use crate::workspace::PaneView;

/// Las medidas del panel en este frame, antes de pintar.
pub(super) struct Frame {
    pub(super) typeface: Typeface,
    pub(super) line_height: Pixels,
    /// El área del texto: sin el margen del tema ni el de los números.
    pub(super) text: Bounds<Pixels>,
    /// Lo que mide una columna de texto.
    pub(super) cell: Pixels,
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
