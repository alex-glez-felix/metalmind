//! Pinta solo las líneas visibles; GPUI cachea el shaping entre frames, así
//! que el scroll solo da forma a las líneas que entran.

use gpui_kit::{
    App, Bounds, ContentMask, ElementId, ElementInputHandler, Entity, Font, GlobalElementId, Hsla,
    InspectorElementId, IntoElement, LayoutId, PaintQuad, Pixels, Point, ShapedLine, SharedString,
    Style, TextAlign, TextRun, UnderlineStyle, Window, fill, point, relative, size,
};
use nota_buffer::{Buffer, Selection};
use nota_vim::Mode;

use super::Editor;
use super::gutter::{Gutter, PaintedGutter};
use super::layout::{FrameLayout, LaidLine, Viewport};
use crate::theme::Theme;
use crate::workspace::PaneView;

pub struct EditorElement {
    editor: Entity<Editor>,
    theme: Theme,
}

impl EditorElement {
    pub fn new(editor: Entity<Editor>, theme: Theme) -> Self {
        Self { editor, theme }
    }
}

pub struct Prepainted {
    layout: FrameLayout,
    gutter: Option<PaintedGutter>,
    selection: Vec<PaintQuad>,
    cursor: Option<PaintQuad>,
    preedit: Option<Preedit>,
}

struct Preedit {
    origin: Point<Pixels>,
    background: PaintQuad,
    shaped: ShapedLine,
}

/// Fuente y tamaño heredados del contenedor, resueltos para este frame.
pub struct Typeface {
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
        let line_height = window.line_height();
        let Some(view) = editor.view(cx) else {
            let viewport = Viewport::new(bounds, line_height, editor.scroll);
            return Prepainted {
                layout: FrameLayout {
                    viewport,
                    lines: Vec::new(),
                },
                gutter: None,
                selection: Vec::new(),
                cursor: None,
                preedit: None,
            };
        };
        let head = view.cursor();
        let moved = (editor.revealed != Some(head)).then(|| view.buffer().line_of(head));
        let mut scroll = editor.scroll;
        let rows = Viewport::new(bounds, line_height, scroll).full_rows();
        scroll.settle(view.buffer().last_line(), moved, rows);
        let gutter = Gutter::new(&view, rows, em_width(&typeface, window));
        let viewport = Viewport::new(gutter.text_bounds(bounds), line_height, scroll);
        let lines = shape_visible_lines(&view, &viewport, &typeface, &self.theme, window);
        let layout = FrameLayout { viewport, lines };
        let painted_gutter = (gutter.columns > 0)
            .then(|| gutter.paint(&view, bounds, &layout, &typeface, &self.theme, window));
        let selection = selection_quads(view.buffer(), view.selection, &layout, &self.theme);
        let cursor = cursor_quad(&view, &layout, &typeface, &self.theme, window);
        let preedit = editor
            .marked
            .clone()
            .filter(|_| view.mode == Mode::Insert)
            .and_then(|text| preedit(&view, &layout, text, &typeface, &self.theme, window));
        self.editor.update(cx, |editor, _| {
            editor.layout = Some(layout.clone());
            editor.scroll = scroll;
            editor.revealed = Some(head);
        });
        Prepainted {
            layout,
            gutter: painted_gutter,
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
            if let Some(gutter) = prepainted.gutter.take() {
                window.paint_quad(gutter.background);
                for (origin, shaped) in &gutter.labels {
                    paint_line(shaped, *origin, line_height, window, cx);
                }
            }
            for quad in prepainted.selection.drain(..) {
                window.paint_quad(quad);
            }
            for laid in &prepainted.layout.lines {
                paint_line(&laid.shaped, laid.origin, line_height, window, cx);
            }
            if let Some(cursor) = prepainted.cursor.take() {
                window.paint_quad(cursor);
            }
            if let Some(preedit) = prepainted.preedit.take() {
                window.paint_quad(preedit.background);
                paint_line(&preedit.shaped, preedit.origin, line_height, window, cx);
            }
        });
        self.editor
            .update(cx, |editor, _| editor.report_first_paint());
    }
}

fn paint_line(
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

fn shape_visible_lines(
    view: &PaneView<'_>,
    viewport: &Viewport,
    typeface: &Typeface,
    theme: &Theme,
    window: &Window,
) -> Vec<LaidLine> {
    let last_line = view.buffer().last_line();
    (viewport.first_line..=last_line)
        .take(viewport.painted_rows())
        .enumerate()
        .filter_map(|(row, line)| {
            let text = view
                .document
                .line_text(line)
                .inspect_err(|error| log::error!("línea {line} ilegible: {error}"))
                .ok()?;
            Some(LaidLine {
                line,
                origin: viewport.row_origin(row),
                shaped: shape(text.into(), typeface, theme.foreground, None, window),
            })
        })
        .collect()
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

fn selection_quads(
    buffer: &Buffer,
    selection: Selection,
    layout: &FrameLayout,
    theme: &Theme,
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
                    theme.selection,
                )
            })
        })
        .collect()
}

/// Bloque en Normal y Visual, barra en Insert; en la línea de comandos el
/// cursor está abajo, en la barra de estado. Un panel sin foco muestra un
/// bloque tenue.
fn cursor_quad(
    view: &PaneView<'_>,
    layout: &FrameLayout,
    typeface: &Typeface,
    theme: &Theme,
    window: &Window,
) -> Option<PaintQuad> {
    let (width, color) = match view.mode {
        Mode::CommandLine => return None,
        Mode::Insert => (theme.caret_width, theme.caret),
        Mode::Normal | Mode::OperatorPending(_) | Mode::Visual(_) => {
            let color = if view.focused {
                theme.block_cursor
            } else {
                theme.unfocused_cursor
            };
            (block_width(view, layout, typeface, window)?, color)
        }
    };
    let (laid, column) = cursor_position(view, layout)?;
    let left = laid.shaped.x_for_index(column);
    Some(fill(
        Bounds::new(
            point(laid.origin.x + left, laid.origin.y),
            size(width, layout.viewport.line_height),
        ),
        color,
    ))
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
    if next > cursor && column < laid.shaped.len() {
        let left = laid.shaped.x_for_index(column);
        Some(
            laid.shaped
                .x_for_index(column + (next.get() - cursor.get()))
                - left,
        )
    } else {
        Some(em_width(typeface, window))
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
    view: &PaneView<'_>,
    layout: &FrameLayout,
    text: String,
    typeface: &Typeface,
    theme: &Theme,
    window: &Window,
) -> Option<Preedit> {
    let (laid, column) = cursor_position(view, layout)?;
    let underline = UnderlineStyle {
        color: Some(theme.foreground),
        thickness: theme.underline_thickness,
        wavy: false,
    };
    let shaped = shape(
        text.into(),
        typeface,
        theme.foreground,
        Some(underline),
        window,
    );
    let origin = point(
        laid.origin.x + laid.shaped.x_for_index(column),
        laid.origin.y,
    );
    let background = fill(
        Bounds::new(origin, size(shaped.width(), layout.viewport.line_height)),
        theme.background,
    );
    Some(Preedit {
        origin,
        background,
        shaped,
    })
}
