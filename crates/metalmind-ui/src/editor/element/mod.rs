//! Pinta solo las líneas visibles; GPUI cachea el shaping entre frames, así
//! que el scroll solo da forma a las líneas que entran.

use gpui_kit::{
    AnyElement, App, AvailableSpace, Bounds, ContentMask, ElementId, ElementInputHandler, Entity,
    GlobalElementId, InspectorElementId, IntoElement, LayoutId, PaintQuad, Pixels, Style, Window,
    point, px, relative, size,
};
use metalmind_syntax::Screen;
use metalmind_vim::Mode;

mod measures;
mod paint;

use self::measures::{Frame, geometry_of, inset, look_of, rows_of, skip_fraction, to_units};
pub use self::paint::shape;
use self::paint::{for_each_text, paint_background, paint_formulas, paint_line};
use super::Editor;
use super::cursor::{Preedit, cursor_quad, em_width, preedit};
use super::formula_tooltip;
use super::gutter::{Gutter, PaintedGutter};
use super::highlight::{search_quads, selection_quads};
use super::layout::{FrameLayout, LaidLine, Viewport};
pub use super::lines::Typeface;
use super::lines::{ImageRequest, MathFrame, Shaped, code_block_quads, shape_visible_lines};
use super::scroll::{Anchor, Scroll, f32_of};
use super::text::Faces;
use crate::frames::FrameClock;
use crate::math::{Formulas, Geometry, Image};
use crate::theme::Theme;
use crate::workspace::{PaneId, Workspace};

pub struct EditorElement {
    editor: Entity<Editor>,
    theme: Theme,
}

impl EditorElement {
    /// Lo que flota sobre el texto de este frame: el tooltip de la mark o
    /// de la fórmula rota bajo el ratón, dentro del área del editor, y la
    /// lista de `#`.
    fn show_overlays(
        &self,
        gutter: &Gutter,
        layout: &FrameLayout,
        clip: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let editor = self.editor.read(cx);
        let mark = editor.shown_mark_tooltip();
        let formula = formula_tooltip::hover_at(Some(layout), editor.pointer);
        if let Some(line) = mark {
            gutter.mark_tooltip(line, layout, clip, &self.theme, window, cx);
        }
        if let Some(hover) = formula {
            formula_tooltip::show(&hover, clip, &self.theme, window, cx);
        }
        let max_width = window.viewport_size().width;
        let completion = self.editor.update(cx, |editor, cx| {
            editor.completion_overlay(layout, max_width, &self.theme, cx)
        });
        if let Some(mut completion) = completion {
            // Va fuera del árbol del editor, así que se mide sola.
            completion.layout_as_root(AvailableSpace::min_size(), window, cx);
            window.defer_draw(completion, window.element_offset(), 1, None);
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
    /// Lo del texto, que sin `wrap` se corre a lo ancho: sin el margen de
    /// los números.
    text_clip: Bounds<Pixels>,
    gutter: Option<PaintedGutter>,
    /// Las zonas de las marks del margen, con su tooltip.
    mark_tooltips: Vec<AnyElement>,
    code_blocks: Vec<PaintQuad>,
    search: Vec<PaintQuad>,
    selection: Vec<PaintQuad>,
    cursor: Option<PaintQuad>,
    preedit: Option<Preedit>,
}

impl Prepainted {
    /// Nada que pintar: el panel no tiene documento, lo que sería un bug.
    fn empty(viewport: Viewport, clip: Bounds<Pixels>) -> Self {
        Self {
            clip,
            text_clip: clip,
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
        }
    }
}

impl IntoElement for EditorElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl EditorElement {
    /// El panel, su workspace y las fórmulas de la ventana.
    fn handles(&self, cx: &App) -> (Entity<Workspace>, PaneId, Entity<Formulas>) {
        let editor = self.editor.read(cx);
        (
            editor.workspace.clone(),
            editor.pane,
            editor.formulas.clone(),
        )
    }

    /// El margen de los números y las marks en un panel de `rows` filas.
    fn gutter(&self, rows: usize, cell: Pixels, cx: &App) -> Option<Gutter> {
        let (workspace, pane, _) = self.handles(cx);
        let marks = workspace
            .read(cx)
            .gutter_marks(pane)
            .inspect_err(|error| log::error!("{error}"))
            .ok()
            .flatten();
        let mark_columns = self.theme.layout.gutter_mark_columns;
        let editor = self.editor.read(cx);
        let view = editor.view(cx)?;
        Some(Gutter::new(&view, marks, mark_columns, rows, cell))
    }

    /// Le da al panel el ancho de su texto, su alto y lo que mide cada
    /// línea en este frame, con el medidor de fórmulas de la letra de hoy.
    fn resize(&self, frame: &Frame, geometry: Geometry, window: &Window, cx: &mut App) {
        let (workspace, pane, formulas) = self.handles(cx);
        let faces = Faces::new(
            frame.typeface.font.clone(),
            frame.typeface.size,
            &self.theme,
        );
        let lines = self
            .editor
            .update(cx, |editor, _| editor.text_screen(faces, window));
        let wrap = self.editor.read(cx).view(cx).is_some_and(|view| view.wrap);
        let screen = Screen {
            width: f32::from(frame.text.size.width),
            wrap,
            column: f32::from(frame.cell),
            lines,
        };
        let measure = formulas.update(cx, |formulas, _| formulas.measure(geometry));
        let measures = rows_of(frame.line_height, &self.theme, measure);
        let height = to_units(f32::from(frame.text.size.height).floor());
        let resized = workspace.update(cx, |workspace, _| {
            workspace.set_screen(pane, screen)?;
            workspace.resize(pane, height, measures)
        });
        if let Err(error) = resized {
            log::error!("{error}");
        }
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
        let typeface = Typeface::of(window);
        let content = inset(bounds, &self.theme);
        let clip = Bounds::new(
            point(bounds.left(), content.top()),
            size(bounds.size.width, content.size.height),
        );
        let line_height = window.line_height();
        let rows = Viewport::new(content, line_height, Scroll::default()).full_rows();
        let geometry = geometry_of(&typeface, line_height, &self.theme, window);
        let (_, _, formulas) = self.handles(cx);
        let cell = em_width(&typeface, window);
        let Some(gutter) = self.gutter(rows, cell, cx) else {
            let viewport = Viewport::new(content, line_height, self.editor.read(cx).scroll);
            return Prepainted::empty(viewport, clip);
        };
        let frame = Frame::new(
            bounds,
            gutter.width,
            typeface,
            line_height,
            cell,
            &self.theme,
        );
        self.resize(&frame, geometry, window, cx);
        let typeface = &frame.typeface;
        let vault_links = self.editor.update(cx, |editor, cx| editor.links(cx));
        let editor = self.editor.read(cx);
        let Some(view) = editor.view(cx) else {
            return Prepainted::empty(Viewport::new(content, line_height, editor.scroll), clip);
        };
        let anchor = Anchor {
            top: view.viewport.top(),
            skip: view.viewport.skip(),
        };
        let hidden = skip_fraction(&view, anchor.top);
        let scroll = editor.scroll.follow(anchor, hidden, editor.synced);
        let viewport = Viewport {
            left: px(f32_of(view.viewport.left())),
            ..Viewport::new(frame.text, line_height, scroll)
        };
        let math = MathFrame {
            formulas: formulas.read(cx),
            geometry,
            look: look_of(geometry, &self.theme, window),
            source: view.source_lines(),
        };
        let Shaped { lines, images } = shape_visible_lines(
            &view,
            &viewport,
            typeface,
            &self.theme,
            &vault_links,
            &math,
            window,
        );
        let code_blocks =
            code_block_quads(&lines, frame.text.left(), frame.column.right(), &self.theme);
        let mut layout = FrameLayout { viewport, lines };
        let painted_gutter = (gutter.columns > 0)
            .then(|| gutter.paint(&view, frame.column, &layout, typeface, &self.theme, window));
        let cells = frame.cells();
        let search = search_quads(&view, &layout, cells, &self.theme);
        let selection = selection_quads(&view, &layout, cells, &self.theme);
        let cursor = cursor_quad(&view, &layout, typeface, &self.theme, window);
        let preedit = editor
            .marked
            .clone()
            .filter(|_| view.mode == Mode::Insert)
            .and_then(|text| preedit(&view, &layout, text, typeface, &self.theme, window));
        attach_images(&formulas, images, &mut layout.lines, cx);
        self.editor.update(cx, |editor, _| {
            editor.layout = Some(layout.clone());
            editor.scroll = scroll;
            editor.synced = Some(anchor);
        });
        // Recortadas como se pinta: una línea a medio scroll no asoma su zona.
        let mark_tooltips = window
            .with_content_mask(Some(ContentMask { bounds: clip }), |window| {
                gutter.mark_zones(&layout, &self.editor, window, cx)
            });
        self.show_overlays(&gutter, &layout, clip, window, cx);
        Prepainted {
            layout,
            clip,
            text_clip: frame.text_clip(clip),
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
        });
        let text_clip = prepainted.text_clip;
        window.with_content_mask(Some(ContentMask { bounds: text_clip }), |window| {
            for quad in prepainted.code_blocks.drain(..) {
                window.paint_quad(quad);
            }
            // El fondo del código inline y del resaltado, bajo la selección.
            for laid in &prepainted.layout.lines {
                for_each_text(laid, |shaped, origin, height| {
                    paint_background(shaped, origin, height, window, cx);
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
                for_each_text(laid, |shaped, origin, height| {
                    paint_line(shaped, origin, height, window, cx);
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
