//! Pinta solo las líneas visibles; GPUI cachea el shaping entre frames, así
//! que el scroll solo da forma a las líneas que entran.

use std::sync::Arc;

use gpui_kit::{
    AnyElement, App, Bounds, ContentMask, Corners, ElementId, ElementInputHandler, Entity,
    GlobalElementId, Hsla, InspectorElementId, IntoElement, LayoutId, PaintQuad, Pixels, Point,
    ShapedLine, SharedString, Style, TextAlign, TextRun, UnderlineStyle, Window, point, px,
    relative, size,
};
use metalmind_vim::{FormulaRows, Mode, Rows};

use super::Editor;
use super::cursor::{Preedit, cursor_quad, em_width, preedit};
use super::formula_tooltip;
use super::gutter::{Gutter, PaintedGutter};
use super::highlight::{Cells, search_quads, selection_quads};
use super::layout::{FrameLayout, LaidLine, Viewport};
pub use super::lines::Typeface;
use super::lines::{ImageRequest, MathFrame, Shaped, code_block_quads, shape_visible_lines};
use super::scroll::Scroll;
use crate::frames::FrameClock;
use crate::math::{Formulas, Geometry, Image, Look};
use crate::theme::Theme;
use crate::workspace::{PaneId, Workspace};

pub struct EditorElement {
    editor: Entity<Editor>,
    theme: Theme,
}

impl EditorElement {
    /// El tooltip de la mark o de la fórmula rota bajo el ratón, dentro del
    /// área del editor.
    fn show_tooltips(
        &self,
        gutter: &Gutter,
        layout: &FrameLayout,
        clip: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let editor = self.editor.read(cx);
        let (mark, formula) = (editor.shown_mark_tooltip(), editor.formula_error.clone());
        if let Some(line) = mark {
            gutter.mark_tooltip(line, layout, clip, &self.theme, window, cx);
        }
        if let Some(hover) = formula {
            formula_tooltip::show(&hover, clip, &self.theme, window, cx);
        }
    }

    pub fn new(editor: Entity<Editor>, theme: Theme) -> Self {
        Self { editor, theme }
    }
}

pub struct Prepainted {
    layout: FrameLayout,
    /// Lo que se puede pintar: todo el ancho y el alto sin el margen
    /// vertical, para que una fila a medio scroll no lo invada.
    clip: Bounds<Pixels>,
    gutter: Option<PaintedGutter>,
    /// Las zonas de las marks del margen, con su tooltip.
    mark_tooltips: Vec<AnyElement>,
    code_blocks: Vec<PaintQuad>,
    search: Vec<PaintQuad>,
    selection: Vec<PaintQuad>,
    cursor: Option<PaintQuad>,
    preedit: Option<Preedit>,
}

impl IntoElement for EditorElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl EditorElement {
    /// Le da al panel su alto y lo que mide cada línea en este frame, con
    /// el medidor de fórmulas de la letra de hoy.
    fn resize(
        &self,
        content: Bounds<Pixels>,
        line_height: Pixels,
        geometry: Geometry,
        cx: &mut App,
    ) -> (Entity<Workspace>, PaneId, Entity<Formulas>) {
        let (workspace, pane, formulas) = {
            let editor = self.editor.read(cx);
            (
                editor.workspace.clone(),
                editor.pane,
                editor.formulas.clone(),
            )
        };
        let measure = formulas.update(cx, |formulas, _| formulas.measure(geometry));
        let measures = rows_of(line_height, &self.theme, measure);
        let height = to_units(f32::from(content.size.height).floor());
        let resized = workspace.update(cx, |workspace, _| workspace.resize(pane, height, measures));
        if let Err(error) = resized {
            log::error!("{error}");
        }
        (workspace, pane, formulas)
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
        let content = inset(bounds, &self.theme);
        let clip = Bounds::new(
            point(bounds.left(), content.top()),
            size(bounds.size.width, content.size.height),
        );
        let line_height = window.line_height();
        let rows = Viewport::new(content, line_height, Scroll::default()).full_rows();
        let geometry = geometry_of(&typeface, line_height, &self.theme, window);
        let (workspace, pane, formulas) = self.resize(content, line_height, geometry, cx);
        let marks = workspace
            .read(cx)
            .gutter_marks(pane)
            .inspect_err(|error| log::error!("{error}"))
            .ok()
            .flatten();
        let vault_links = self.editor.update(cx, |editor, cx| editor.links(cx));
        let editor = self.editor.read(cx);
        let Some(view) = editor.view(cx) else {
            let viewport = Viewport::new(content, line_height, editor.scroll);
            return Prepainted {
                clip,
                layout: FrameLayout {
                    viewport,
                    lines: Vec::new(),
                },
                gutter: None,
                mark_tooltips: Vec::new(),
                code_blocks: Vec::new(),
                search: Vec::new(),
                selection: Vec::new(),
                cursor: None,
                preedit: None,
            };
        };
        let top = view.viewport.top();
        let scroll = editor.scroll.follow(top, editor.synced_top);
        let cell = em_width(&typeface, window);
        let gutter = Gutter::new(&view, marks, self.theme.gutter_mark_columns, rows, cell);
        let viewport = Viewport::new(gutter.text_bounds(content), line_height, scroll);
        let math = MathFrame {
            formulas: formulas.read(cx),
            geometry,
            look: look_of(geometry, &self.theme, window),
            source: view.source_lines(),
        };
        let Shaped { lines, images } = shape_visible_lines(
            &view,
            &viewport,
            &typeface,
            &self.theme,
            &vault_links,
            &math,
            window,
        );
        let code_blocks = code_block_quads(&lines, bounds.right(), &self.theme);
        let mut layout = FrameLayout { viewport, lines };
        let painted_gutter = (gutter.columns > 0)
            .then(|| gutter.paint(&view, bounds, &layout, &typeface, &self.theme, window));
        let cells = Cells {
            width: cell,
            edge: bounds.right(),
        };
        let search = search_quads(&view, &layout, cells, &self.theme);
        let selection = selection_quads(&view, &layout, cells, &self.theme);
        let cursor = cursor_quad(&view, &layout, &typeface, &self.theme, window);
        let preedit = editor
            .marked
            .clone()
            .filter(|_| view.mode == Mode::Insert)
            .and_then(|text| preedit(&view, &layout, text, &typeface, &self.theme, window));
        attach_images(&formulas, images, &mut layout.lines, cx);
        self.editor.update(cx, |editor, _| {
            editor.layout = Some(layout.clone());
            editor.scroll = scroll;
            editor.synced_top = Some(top);
        });
        // Recortadas como se pinta: una línea a medio scroll no asoma su zona.
        let mark_tooltips = window
            .with_content_mask(Some(ContentMask { bounds: clip }), |window| {
                gutter.mark_zones(&layout, &self.editor, window, cx)
            });
        self.show_tooltips(&gutter, &layout, clip, window, cx);
        Prepainted {
            layout,
            clip,
            gutter: painted_gutter,
            mark_tooltips,
            code_blocks,
            search,
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
        let (focus_handle, formulas) = {
            let editor = self.editor.read(cx);
            (editor.focus_handle.clone(), editor.formulas.clone())
        };
        formulas.update(cx, |formulas, _| formulas.drop_evicted(window));
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.editor.clone()),
            cx,
        );
        let clip = prepainted.clip;
        window.with_content_mask(Some(ContentMask { bounds: clip }), |window| {
            if let Some(gutter) = prepainted.gutter.take() {
                window.paint_quad(gutter.background);
                for label in &gutter.labels {
                    paint_line(&label.shaped, label.origin, label.height, window, cx);
                }
            }
            for zone in &mut prepainted.mark_tooltips {
                zone.paint(window, cx);
            }
            for quad in prepainted.code_blocks.drain(..) {
                window.paint_quad(quad);
            }
            // El fondo del código inline y del resaltado, bajo la selección.
            for laid in &prepainted.layout.lines {
                for_each_text(laid, |shaped, origin| {
                    paint_background(shaped, origin, laid.text_height, window, cx);
                });
            }
            for quad in prepainted
                .search
                .drain(..)
                .chain(prepainted.selection.drain(..))
            {
                window.paint_quad(quad);
            }
            for laid in &prepainted.layout.lines {
                for_each_text(laid, |shaped, origin| {
                    paint_line(shaped, origin, laid.text_height, window, cx);
                });
                paint_formulas(laid, window);
            }
            if let Some(cursor) = prepainted.cursor.take() {
                window.paint_quad(cursor);
            }
            if let Some(preedit) = prepainted.preedit.take() {
                window.paint_quad(preedit.background);
                paint_line(&preedit.shaped, preedit.origin, preedit.height, window, cx);
            }
        });
        FrameClock::painted(cx);
    }
}

/// El área del texto y los números, sin el margen del tema.
fn inset(bounds: Bounds<Pixels>, theme: &Theme) -> Bounds<Pixels> {
    let (x, y) = (theme.text_padding_x, theme.text_padding_y);
    Bounds::new(
        point(bounds.left() + x, bounds.top() + y),
        size(
            (bounds.size.width - x * 2.).max(px(0.)),
            (bounds.size.height - y * 2.).max(px(0.)),
        ),
    )
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

fn paint_background(
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

/// Lo que mide una línea de texto, cada heading y cada fórmula, en píxeles
/// enteros: las mismas medidas para el motor, que mueve la ventana, y para
/// la vista.
fn rows_of(line_height: Pixels, theme: &Theme, formulas: Arc<dyn FormulaRows>) -> Rows {
    let text = f32::from(line_height);
    Rows {
        text: to_units(text.round()),
        headings: theme
            .markdown
            .headings
            .map(|(scale, _)| to_units((text * scale).round())),
        formulas: Some(formulas),
    }
}

/// Las medidas de la letra del editor con que se componen las fórmulas.
fn geometry_of(
    typeface: &Typeface,
    line_height: Pixels,
    theme: &Theme,
    window: &Window,
) -> Geometry {
    let text_system = window.text_system();
    let font_id = text_system.resolve_font(&typeface.font);
    Geometry {
        size: f32::from(typeface.size) * theme.markdown.math_size,
        line_height: f32::from(line_height),
        ascent: f32::from(text_system.ascent(font_id, typeface.size)).abs(),
        descent: f32::from(text_system.descent(font_id, typeface.size)).abs(),
        block_margin: f32::from(theme.markdown.math_block_margin),
    }
}

/// Una fórmula se pinta con la letra de las fórmulas, la escala de la
/// pantalla, el color del texto y el margen del tema.
fn look_of(geometry: Geometry, theme: &Theme, window: &Window) -> Look {
    let color = theme.foreground.to_rgb();
    Look {
        size: geometry.size,
        scale: window.scale_factor(),
        color: [color.r, color.g, color.b, color.a],
        padding: f32::from(theme.markdown.math_padding),
    }
}

/// Las imágenes que esperan las fórmulas visibles: las que ya están se
/// ponen en su lugar y las demás se mandan a pintar al fondo.
fn attach_images(
    formulas: &Entity<Formulas>,
    requests: Vec<ImageRequest>,
    lines: &mut [LaidLine],
    cx: &mut App,
) {
    formulas.update(cx, |formulas, cx| {
        for request in requests {
            let Image::Ready(image) = formulas.image(&request.key, cx) else {
                continue;
            };
            if let Some(placed) = lines
                .get_mut(request.line)
                .and_then(|laid| laid.formulas.get_mut(request.formula))
            {
                placed.image = Some(image);
            }
        }
    });
}

/// Cada trozo de texto de la línea con su origen.
fn for_each_text(laid: &LaidLine, mut paint: impl FnMut(&ShapedLine, Point<Pixels>)) {
    for piece in &laid.pieces {
        if let Some(text) = &piece.text {
            paint(text, point(laid.origin.x + piece.x, laid.origin.y));
        }
    }
}

fn paint_formulas(laid: &LaidLine, window: &mut Window) {
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

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "píxeles de un panel, ya redondeados y nunca negativos"
)]
fn to_units(pixels: f32) -> usize {
    pixels.max(1.) as usize
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
