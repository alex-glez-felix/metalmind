//! El mensaje de RaTeX sobre una fórmula que no compone, al pasar el ratón,
//! como en Obsidian: un tooltip como los demás (`crate::tooltip`), con el
//! texto en el color de error.

use gpui_kit::{
    AnyElement, IntoElement as _, ParentElement as _, Pixels, Point, SharedString, Styled as _,
    anchored, deferred, div, point, relative,
};

use super::layout::FormulaError;
use crate::theme::Theme;

#[derive(Clone, Debug, PartialEq)]
pub struct Hover {
    message: SharedString,
    /// Debajo de la fórmula, donde no tapa lo que se está leyendo.
    position: Point<Pixels>,
}

impl Hover {
    pub fn new(error: &FormulaError, mouse: Point<Pixels>) -> Self {
        Self {
            message: error.message.clone(),
            position: point(mouse.x, error.bounds.bottom()),
        }
    }
}

pub fn tooltip(hover: &Hover, theme: &Theme) -> AnyElement {
    deferred(
        anchored().position(hover.position).snap_to_window().child(
            div()
                .font_family(theme.font_family.clone())
                .text_size(theme.ui_font_size)
                .line_height(relative(theme.ui_line_height))
                .px(theme.tooltip_padding_x)
                .py(theme.tooltip_padding_y)
                .bg(theme.menu_background)
                .border(theme.border_width)
                .border_color(theme.menu_border)
                .rounded(theme.radius_md)
                .text_color(theme.error)
                .child(hover.message.clone()),
        ),
    )
    .with_priority(1)
    .into_any_element()
}
