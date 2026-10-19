//! Las líneas visibles con su estilo: dónde va cada una según lo que mide,
//! con qué letra y en qué colores, y el fondo de los bloques de código.

use gpui_kit::{Bounds, Font, PaintQuad, Pixels, SharedString, Window, fill, point, px, size};
use metalmind_syntax::LineKind;
use metalmind_vim::{Layout, LineHeights};

use super::layout::{LaidLine, Viewport};
use super::links::Links;
use super::markdown::runs;
use super::rows::place;
use super::text::{line_font, text_runs};
use crate::theme::Theme;
use crate::workspace::PaneView;

/// La letra del editor, resuelta para este frame.
pub struct Typeface {
    pub font: Font,
    pub size: Pixels,
}

/// Lo que mide la línea `line` en píxeles: del layout del documento o, sin
/// él, una línea de texto.
pub fn line_height(view: &PaneView<'_>, line_height: Pixels, line: usize) -> f32 {
    match view.document.layout() {
        Some(layout) => units(layout.height(line)),
        None => f32::from(line_height),
    }
}

fn units(value: usize) -> f32 {
    f32::from(u16::try_from(value).unwrap_or(u16::MAX))
}

pub fn shape_visible_lines(
    view: &PaneView<'_>,
    viewport: &Viewport,
    typeface: &Typeface,
    theme: &Theme,
    links: &Links,
    window: &Window,
) -> Vec<LaidLine> {
    let styles = view.document.layout().map(Layout::styles);
    let height = f32::from(viewport.bounds.size.height);
    let placed = place(viewport.scroll, view.buffer().last_line(), height, |line| {
        line_height(view, viewport.line_height, line)
    });
    placed
        .into_iter()
        .filter_map(|placed| {
            let text = view
                .document
                .line_text(placed.line)
                .inspect_err(|error| log::error!("línea {} ilegible: {error}", placed.line))
                .ok()?;
            let style = styles.and_then(|styles| styles.line(placed.line));
            let kind = style.map(|style| style.kind);
            let (font_size, weight) = line_font(kind, typeface.size, theme);
            let runs = runs(text.len(), style, |spot| links.resolves(spot));
            let runs = text_runs(&runs, kind, &typeface.font, weight, theme);
            let shaped =
                window
                    .text_system()
                    .shape_line(SharedString::from(text), font_size, &runs, None);
            Some(LaidLine {
                line: placed.line,
                origin: point(
                    viewport.bounds.left(),
                    viewport.bounds.top() + px(placed.top),
                ),
                height: px(placed.height),
                font_size,
                shaped,
                code_block: matches!(kind, Some(LineKind::Fence(_))),
            })
        })
        .collect()
}

/// El fondo de cada línea de un bloque de código, hasta `right`.
pub fn code_block_quads(lines: &[LaidLine], right: Pixels, theme: &Theme) -> Vec<PaintQuad> {
    lines
        .iter()
        .filter(|laid| laid.code_block)
        .map(|laid| {
            fill(
                Bounds::new(laid.origin, size(right - laid.origin.x, laid.height)),
                theme.markdown.code_block_background,
            )
        })
        .collect()
}
