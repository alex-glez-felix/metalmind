//! Lo que pinta el editor ya armado: el texto de cada fila y las fórmulas.

use gpui_kit::{
    App, Corners, Hsla, Pixels, Point, ShapedLine, SharedString, TextAlign, TextRun,
    UnderlineStyle, Window, point,
};

use crate::editor::lines::Typeface;

use crate::editor::layout::LaidLine;

pub(super) fn paint_line(
    shaped: &ShapedLine,
    origin: Point<Pixels>,
    line_height: Pixels,
    window: &mut Window,
    cx: &mut App,
) {
    if let Err(error) = shaped.paint(origin, line_height, TextAlign::Left, None, window, cx) {
        log::error!("no se pudo pintar una línea: {error:#}");
    }
}

pub(super) fn paint_background(
    shaped: &ShapedLine,
    origin: Point<Pixels>,
    line_height: Pixels,
    window: &mut Window,
    cx: &mut App,
) {
    if let Err(error) =
        shaped.paint_background(origin, line_height, TextAlign::Left, None, window, cx)
    {
        log::error!("no se pudo pintar el fondo de una línea: {error:#}");
    }
}

/// Cada trozo de texto de la línea con su origen y el alto de su fila.
pub(super) fn for_each_text(
    laid: &LaidLine,
    mut paint: impl FnMut(&ShapedLine, Point<Pixels>, Pixels),
) {
    for row in &laid.rows {
        let origin = laid.row_origin(row);
        for piece in &row.pieces {
            if let Some(text) = &piece.text {
                paint(text, point(origin.x + piece.x, origin.y), row.height);
            }
        }
    }
}

pub(super) fn paint_formulas(laid: &LaidLine, window: &mut Window) {
    for formula in &laid.formulas {
        let Some(image) = formula.image.clone() else {
            continue;
        };
        let bounds = formula.bounds;
        if let Err(error) = window.paint_image(bounds, bounds, Corners::default(), image, 0, false)
        {
            log::error!("no se pudo pintar una fórmula: {error:#}");
        }
    }
}

pub fn shape(
    text: SharedString,
    typeface: &Typeface,
    color: Hsla,
    underline: Option<UnderlineStyle>,
    window: &Window,
) -> ShapedLine {
    let run = TextRun {
        len: text.len(),
        font: typeface.font.clone(),
        color,
        background_color: None,
        underline,
        strikethrough: None,
    };
    window
        .text_system()
        .shape_line(text, typeface.size, &[run], None)
}
