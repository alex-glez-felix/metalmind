//! El cursor de cada modo y el texto provisional del IME, sobre la línea
//! pintada del cursor: miden lo que mide esa línea.

use gpui_kit::{Bounds, PaintQuad, Pixels, Point, ShapedLine, UnderlineStyle, Window, fill, size};
use metalmind_vim::Mode;

use super::element::shape;
use super::layout::{FrameLayout, LaidLine};
use super::lines::Typeface;
use crate::theme::Theme;
use crate::workspace::PaneView;

pub struct Preedit {
    pub origin: Point<Pixels>,
    pub height: Pixels,
    pub background: PaintQuad,
    pub shaped: ShapedLine,
}

/// Bloque en Normal y Visual, barra en Insert; en la línea de comandos el
/// cursor está abajo, en la barra de estado. Un panel sin foco muestra un
/// bloque tenue.
pub fn cursor_quad(
    view: &PaneView<'_>,
    layout: &FrameLayout,
    typeface: &Typeface,
    theme: &Theme,
    window: &Window,
) -> Option<PaintQuad> {
    let (width, color) = match view.mode {
        Mode::CommandLine => return None,
        Mode::Insert => (theme.stroke.caret, theme.cursor.insert),
        Mode::Normal | Mode::OperatorPending(_) | Mode::Visual(_) => {
            let color = match view.mode {
                _ if !view.focused => theme.cursor.unfocused,
                Mode::Visual(_) => theme.cursor.visual,
                _ => theme.cursor.normal,
            };
            (block_width(view, layout, typeface, window)?, color)
        }
    };
    let (laid, column) = cursor_position(view, layout)?;
    let (origin, height) = laid.place(column)?;
    Some(fill(Bounds::new(origin, size(width, height)), color))
}

/// Ancho del carácter bajo el cursor; sobre el fin de línea, el de una `M`.
fn block_width(
    view: &PaneView<'_>,
    layout: &FrameLayout,
    typeface: &Typeface,
    window: &Window,
) -> Option<Pixels> {
    let (laid, column) = cursor_position(view, layout)?;
    let cursor = view.cursor();
    let next = view.buffer().next_boundary(cursor);
    if next > cursor && column < laid.len() {
        let row = laid.row_of(column)?;
        let left = row.x_for_index(column);
        Some(row.x_for_end(column + (next.get() - cursor.get())) - left)
    } else {
        Some(em_width(&line_face(typeface, laid), window))
    }
}

/// La letra de `laid`: la del editor con el tamaño de esa línea.
fn line_face(typeface: &Typeface, laid: &LaidLine) -> Typeface {
    Typeface {
        font: typeface.font.clone(),
        size: laid.font_size,
    }
}

/// La línea pintada del cursor y su columna en bytes dentro de ella.
pub fn cursor_position<'a>(
    view: &PaneView<'_>,
    layout: &'a FrameLayout,
) -> Option<(&'a LaidLine, usize)> {
    let cursor = view.cursor();
    let buffer = view.buffer();
    let line = buffer.line_of(cursor);
    let laid = layout.line(line)?;
    let column = cursor.get() - buffer.line_start(line).get();
    Some((laid, column))
}

pub fn em_width(typeface: &Typeface, window: &Window) -> Pixels {
    let text_system = window.text_system();
    let font_id = text_system.resolve_font(&typeface.font);
    text_system
        .em_advance(font_id, typeface.size)
        .inspect_err(|error| log::error!("sin ancho de M para el cursor: {error:#}"))
        .unwrap_or(typeface.size)
}

/// El texto provisional del IME o de una tecla muerta, subrayado sobre el
/// cursor; no entra al buffer hasta el commit.
pub fn preedit(
    view: &PaneView<'_>,
    layout: &FrameLayout,
    text: String,
    typeface: &Typeface,
    theme: &Theme,
    window: &Window,
) -> Option<Preedit> {
    let (laid, column) = cursor_position(view, layout)?;
    let underline = UnderlineStyle {
        color: Some(theme.ui.foreground),
        thickness: theme.stroke.text_line,
        wavy: false,
    };
    let shaped = shape(
        text.into(),
        &line_face(typeface, laid),
        theme.ui.foreground,
        Some(underline),
        window,
    );
    let (origin, height) = laid.place(column)?;
    let background = fill(
        Bounds::new(origin, size(shaped.width(), height)),
        theme.ui.background,
    );
    Some(Preedit {
        origin,
        height,
        background,
        shaped,
    })
}
