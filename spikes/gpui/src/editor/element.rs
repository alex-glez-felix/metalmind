//! Pinta solo las líneas visibles; GPUI cachea el shaping entre frames, así
//! que el scroll solo da forma a las líneas que entran.

use gpui_kit::{
    App, Bounds, ContentMask, ElementId, ElementInputHandler, Entity, Font, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, PaintQuad, Pixels, ShapedLine, SharedString, Style,
    TextAlign, TextRun, UnderlineStyle, Window, fill, point, relative, size,
};
use nota_buffer::{Buffer, Selection};
use nota_vim::Mode;

use super::VimEditor;
use super::layout::{FrameLayout, LaidLine, Viewport};
use crate::document::Document;
use crate::tokens::Tokens;

pub struct EditorElement {
    editor: Entity<VimEditor>,
    tokens: Tokens,
}

impl EditorElement {
    pub fn new(editor: Entity<VimEditor>, tokens: Tokens) -> Self {
        Self { editor, tokens }
    }
}

pub struct Prepainted {
    layout: FrameLayout,
    selection: Vec<PaintQuad>,
    cursor: Option<PaintQuad>,
    preedit: Option<Preedit>,
}

struct Preedit {
    origin: gpui_kit::Point<Pixels>,
    background: PaintQuad,
    shaped: ShapedLine,
}

/// Fuente y tamaño heredados del contenedor, resueltos para este frame.
struct Typeface {
    font: Font,
    size: Pixels,
}

impl IntoElement for EditorElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl gpui_kit::Element for EditorElement {
    type RequestLayoutState = ();
    type PrepaintState = Prepainted;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let text_style = window.text_style();
        let typeface = Typeface {
            font: text_style.font(),
            size: text_style.font_size.to_pixels(window.rem_size()),
        };
        let editor = self.editor.read(cx);
        let viewport = Viewport::new(bounds, window.line_height(), editor.scroll);
        let document = &editor.document;
        let lines = shape_visible_lines(document, &viewport, &typeface, &self.tokens, window);
        let layout = FrameLayout { viewport, lines };
        let selection = selection_quads(
            document.buffer(),
            document.selection(),
            &layout,
            &self.tokens,
        );
        let cursor = cursor_quad(document, &layout, &typeface, &self.tokens, window);
        let preedit = editor
            .marked
            .clone()
            .and_then(|text| preedit(document, &layout, text, &typeface, &self.tokens, window));
        self.editor
            .update(cx, |editor, _| editor.layout = Some(layout.clone()));
        Prepainted {
            layout,
            selection,
            cursor,
            preedit,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepainted: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.editor.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.editor.clone()),
            cx,
        );
        let line_height = prepainted.layout.viewport.line_height;
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            for quad in prepainted.selection.drain(..) {
                window.paint_quad(quad);
            }
            for laid in &prepainted.layout.lines {
                paint_line(&laid.shaped, laid.origin, line_height, window, cx);
            }
            if focus_handle.is_focused(window)
                && let Some(cursor) = prepainted.cursor.take()
            {
                window.paint_quad(cursor);
            }
            if let Some(preedit) = prepainted.preedit.take() {
                window.paint_quad(preedit.background);
                paint_line(&preedit.shaped, preedit.origin, line_height, window, cx);
            }
        });
    }
}

fn paint_line(
    shaped: &ShapedLine,
    origin: gpui_kit::Point<Pixels>,
    line_height: Pixels,
    window: &mut Window,
    cx: &mut App,
) {
    if let Err(error) = shaped.paint(origin, line_height, TextAlign::Left, None, window, cx) {
        log::error!("no se pudo pintar una línea: {error:#}");
    }
}

fn shape_visible_lines(
    document: &Document,
    viewport: &Viewport,
    typeface: &Typeface,
    tokens: &Tokens,
    window: &Window,
) -> Vec<LaidLine> {
    let last_line = document.buffer().last_line();
    (viewport.first_line..=last_line)
        .take(viewport.painted_rows())
        .enumerate()
        .filter_map(|(row, line)| {
            let text = document
                .line_text(line)
                .inspect_err(|error| log::error!("línea {line} ilegible: {error}"))
                .ok()?;
            Some(LaidLine {
                line,
                origin: viewport.row_origin(row),
                shaped: shape(text.into(), typeface, tokens, None, window),
            })
        })
        .collect()
}

fn shape(
    text: SharedString,
    typeface: &Typeface,
    tokens: &Tokens,
    underline: Option<UnderlineStyle>,
    window: &Window,
) -> ShapedLine {
    let run = TextRun {
        len: text.len(),
        font: typeface.font.clone(),
        color: tokens.foreground,
        background_color: None,
        underline,
        strikethrough: None,
    };
    window
        .text_system()
        .shape_line(text, typeface.size, &[run], None)
}

fn selection_quads(
    buffer: &Buffer,
    selection: Selection,
    layout: &FrameLayout,
    tokens: &Tokens,
) -> Vec<PaintQuad> {
    if selection.is_cursor() {
        return Vec::new();
    }
    let range = selection.range();
    let line_height = layout.viewport.line_height;
    layout
        .lines
        .iter()
        .filter_map(|laid| {
            let line_start = buffer.line_start(laid.line);
            let start = range.start.max(line_start);
            let end = range.end.min(buffer.line_end(laid.line));
            (start < end).then(|| {
                let left = laid.shaped.x_for_index(start.get() - line_start.get());
                let right = laid.shaped.x_for_index(end.get() - line_start.get());
                fill(
                    Bounds::new(
                        point(laid.origin.x + left, laid.origin.y),
                        size(right - left, line_height),
                    ),
                    tokens.selection,
                )
            })
        })
        .collect()
}

/// Bloque en Normal, barra en Insert.
fn cursor_quad(
    document: &Document,
    layout: &FrameLayout,
    typeface: &Typeface,
    tokens: &Tokens,
    window: &Window,
) -> Option<PaintQuad> {
    let (laid, column) = cursor_position(document, layout)?;
    let left = laid.shaped.x_for_index(column);
    let (width, color) = if document.mode() == Mode::Insert {
        (tokens.caret_width, tokens.caret)
    } else {
        let next = document.buffer().next_boundary(document.cursor());
        let width = if next > document.cursor() && column < laid.shaped.len() {
            laid.shaped
                .x_for_index(column + (next.get() - document.cursor().get()))
                - left
        } else {
            em_width(typeface, window)
        };
        (width, tokens.block_cursor)
    };
    Some(fill(
        Bounds::new(
            point(laid.origin.x + left, laid.origin.y),
            size(width, layout.viewport.line_height),
        ),
        color,
    ))
}

/// La línea pintada del cursor y su columna en bytes dentro de ella.
pub fn cursor_position<'a>(
    document: &Document,
    layout: &'a FrameLayout,
) -> Option<(&'a LaidLine, usize)> {
    let cursor = document.cursor();
    let buffer = document.buffer();
    let line = buffer.line_of(cursor);
    let laid = layout.line(line)?;
    let column = cursor.get() - buffer.line_start(line).get();
    Some((laid, column))
}

fn em_width(typeface: &Typeface, window: &Window) -> Pixels {
    let text_system = window.text_system();
    let font_id = text_system.resolve_font(&typeface.font);
    text_system
        .em_advance(font_id, typeface.size)
        .inspect_err(|error| log::error!("sin ancho de M para el cursor: {error:#}"))
        .unwrap_or(typeface.size)
}

/// El texto provisional del IME o de una tecla muerta, subrayado sobre el
/// cursor; no entra al buffer hasta el commit.
fn preedit(
    document: &Document,
    layout: &FrameLayout,
    text: String,
    typeface: &Typeface,
    tokens: &Tokens,
    window: &Window,
) -> Option<Preedit> {
    let (laid, column) = cursor_position(document, layout)?;
    let underline = UnderlineStyle {
        color: Some(tokens.foreground),
        thickness: tokens.underline_thickness,
        wavy: false,
    };
    let shaped = shape(text.into(), typeface, tokens, Some(underline), window);
    let origin = point(
        laid.origin.x + laid.shaped.x_for_index(column),
        laid.origin.y,
    );
    let background = fill(
        Bounds::new(origin, size(shaped.width(), layout.viewport.line_height)),
        tokens.background,
    );
    Some(Preedit {
        origin,
        background,
        shaped,
    })
}
