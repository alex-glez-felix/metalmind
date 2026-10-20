//! El mensaje de RaTeX sobre una fórmula que no compone, al pasar el ratón,
//! como en Obsidian: un tooltip como el de las marks, con el texto en el
//! color de error, debajo de la fórmula o arriba si abajo no cabe en el
//! editor.

use std::rc::Rc;

use gpui_kit::{
    AnyTooltip, App, AvailableSpace, Bounds, IntoElement as _, Pixels, Point, SharedString, Window,
    point,
};

use super::gutter::{Row, tooltip_origin};
use super::layout::{FormulaError, FrameLayout};
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

/// El mensaje de la fórmula que no compone bajo `pointer` en `layout`, lo
/// que se pinta: se busca en cada frame, así que si la nota, el scroll o el
/// panel cambian con el ratón quieto, no queda el de lo que ya no se ve.
pub fn hover_at(layout: Option<&FrameLayout>, pointer: Option<Point<Pixels>>) -> Option<Hover> {
    layout?.error_at(pointer?).map(Hover::new)
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
    let gap = theme.stroke.border;
    window.set_tooltip(AnyTooltip {
        view,
        mouse_position: origin - point(gap, gap),
        check_visible_and_update: Rc::new(|_, _, _| true),
    });
}

#[cfg(test)]
mod tests {
    use gpui_kit::{px, size};

    use super::*;
    use crate::editor::layout::{LaidLine, Viewport};
    use crate::editor::scroll::Scroll;

    /// Una línea pintada, con una fórmula rota en `errors`.
    fn painted(errors: Vec<FormulaError>) -> FrameLayout {
        let bounds = Bounds::new(point(px(0.), px(0.)), size(px(200.), px(100.)));
        FrameLayout {
            viewport: Viewport::new(bounds, px(10.), Scroll::default()),
            lines: vec![LaidLine {
                line: 0,
                origin: point(px(0.), px(0.)),
                height: px(10.),
                text_height: px(10.),
                font_size: px(10.),
                pieces: Vec::new(),
                formulas: Vec::new(),
                errors,
                code_block: false,
            }],
        }
    }

    #[test]
    fn el_mensaje_sale_de_lo_que_se_pinta_ahora_no_del_frame_de_antes() {
        let broken = FormulaError {
            bounds: Bounds::new(point(px(20.), px(0.)), size(px(40.), px(10.))),
            message: "ParseError".into(),
        };
        let with_formula = painted(vec![broken]);
        let pointer = Some(point(px(30.), px(5.)));
        assert!(hover_at(Some(&with_formula), pointer).is_some());
        assert_eq!(
            hover_at(Some(&with_formula), Some(point(px(5.), px(5.)))),
            None
        );
        // Otra nota en el panel, con el ratón quieto: ya no hay fórmula ahí.
        assert_eq!(hover_at(Some(&painted(Vec::new())), pointer), None);
        assert_eq!(hover_at(Some(&with_formula), None), None, "arrastrando");
    }
}
