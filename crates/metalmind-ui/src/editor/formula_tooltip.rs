//! El mensaje de RaTeX sobre una fórmula que no compone, al pasar el ratón,
//! como en Obsidian: un tooltip como el de las marks, con el texto en el
//! color de error, debajo de la fórmula o arriba si abajo no cabe en el
//! editor.

use std::rc::Rc;

use gpui_kit::{
    AnyTooltip, App, AvailableSpace, Bounds, IntoElement as _, Pixels, SharedString, Window, point,
};

use super::gutter::{Row, tooltip_origin};
use super::layout::FormulaError;
use crate::theme::Theme;
use crate::tooltip::Tooltip;

#[derive(Clone, Debug, PartialEq)]
pub struct Hover {
    message: SharedString,
    /// La fila de la fórmula, desde su principio: el tooltip no la tapa y
    /// no se mueve con el ratón.
    row: Row,
}

impl Hover {
    pub fn new(error: &FormulaError) -> Self {
        Self {
            message: error.message.clone(),
            row: Row {
                top: error.bounds.top(),
                bottom: error.bounds.bottom(),
                text_left: error.bounds.left(),
            },
        }
    }
}

/// Muestra el tooltip de `hover` dentro de `clip`, el área del editor: no
/// tapa la barra de estado ni sale de la ventana.
pub fn show(hover: &Hover, clip: Bounds<Pixels>, theme: &Theme, window: &mut Window, cx: &mut App) {
    let view = Tooltip::error_view(hover.message.clone(), cx);
    let measured =
        view.clone()
            .into_any_element()
            .layout_as_root(AvailableSpace::min_size(), window, cx);
    let origin = tooltip_origin(
        hover.row,
        measured,
        clip.bottom(),
        window.viewport_size().width,
    );
    // GPUI corre el tooltip un píxel del ancla, el grosor del borde.
    let gap = theme.border_width;
    window.set_tooltip(AnyTooltip {
        view,
        mouse_position: origin - point(gap, gap),
        check_visible_and_update: Rc::new(|_, _, _| true),
    });
}
