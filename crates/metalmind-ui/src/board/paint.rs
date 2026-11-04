//! Pinta un plan: cada malla, ya teselada en coordenadas de la pizarra, solo
//! pasa por la cámara.

use gpui_kit::{
    App, BorderStyle, Bounds, Hsla, Path, Pixels, Point as ScreenPoint, TextRun, Window, fill,
    font, point, px, quad, size,
};
use metalmind_board::{Guide, Point, Rect};

use super::camera::Camera;
use super::label;
use super::plan::{Look, Plan};
use super::scene::ElementScene;
use super::shapes::{self, Mesh};
use crate::theme::Theme;

const HALF: f32 = 0.5;

/// Dónde y con qué se pinta un frame.
#[derive(Clone, Copy)]
pub struct Frame<'a> {
    pub camera: Camera,
    pub origin: ScreenPoint<Pixels>,
    pub theme: &'a Theme,
}

impl Frame<'_> {
    pub fn on_screen(&self, p: Point) -> ScreenPoint<Pixels> {
        let at = self.camera.to_screen(p);
        point(self.origin.x + px(at.x), self.origin.y + px(at.y))
    }

    pub fn bounds_of(&self, rect: Rect) -> Bounds<Pixels> {
        let zoom = self.camera.zoom;
        Bounds::new(
            self.on_screen(Point::new(rect.x, rect.y)),
            size(px(rect.w * zoom), px(rect.h * zoom)),
        )
    }
}

/// La línea de una flecha que se está sacando.
pub fn paint_link_preview(from: Point, to: Point, frame: &Frame<'_>, window: &mut Window) {
    let width = f32::from(frame.theme.stroke.board_line);
    if let Ok(mesh) = shapes::stroke(&shapes::polyline(&[from, to], false), width) {
        paint_mesh(&mesh, frame.theme.ui.board_selection, frame, window);
    }
}

/// Un trazo a mano mientras se dibuja.
pub fn paint_stroke_preview(points: &[Point], frame: &Frame<'_>, window: &mut Window) {
    let width = f32::from(frame.theme.stroke.board_line);
    if let Ok(mesh) = shapes::stroke(&shapes::smooth(points), width) {
        paint_mesh(&mesh, frame.theme.ui.board_line, frame, window);
    }
}

/// Una flecha elegida: su recorrido entero en el color de la selección.
pub fn paint_edge_selection(element: &ElementScene, frame: &Frame<'_>, window: &mut Window) {
    for piece in &element.pieces {
        paint_mesh(&piece.mesh, frame.theme.ui.board_selection, frame, window);
    }
}

/// Las guías, de lado a lado de lo que alinean y del mismo grosor a cualquier zoom.
pub fn paint_guides(guides: &[Guide], frame: &Frame<'_>, window: &mut Window) {
    let theme = frame.theme;
    let width = theme.stroke.board_guide;
    for guide in guides {
        let (start, end) = match *guide {
            Guide::Vertical { x, from, to } => (Point::new(x, from), Point::new(x, to)),
            Guide::Horizontal { y, from, to } => (Point::new(from, y), Point::new(to, y)),
        };
        let (a, b) = (frame.on_screen(start), frame.on_screen(end));
        let line = Bounds::new(a, size((b.x - a.x).max(width), (b.y - a.y).max(width)));
        window.paint_quad(fill(line, theme.ui.board_guide));
    }
}

/// Lo que se está dibujando o el rectángulo de selección: solo su contorno.
pub fn paint_outline(rect: Rect, frame: &Frame<'_>, window: &mut Window) {
    let theme = frame.theme;
    window.paint_quad(quad(
        frame.bounds_of(rect),
        Pixels::default(),
        Hsla::default(),
        theme.stroke.board_guide,
        theme.ui.board_selection,
        BorderStyle::default(),
    ));
}

/// Los cuadritos en las esquinas de un nodo elegido, para cambiar su tamaño.
pub fn paint_handles(outline: Rect, frame: &Frame<'_>, window: &mut Window) {
    let theme = frame.theme;
    let handle = theme.layout.board_handle_size;
    let corners = [
        Point::new(outline.x, outline.y),
        Point::new(outline.x + outline.w, outline.y),
        Point::new(outline.x, outline.y + outline.h),
        Point::new(outline.x + outline.w, outline.y + outline.h),
    ];
    for corner in corners {
        let at = frame.on_screen(corner);
        let bounds = Bounds::new(
            point(at.x - handle * HALF, at.y - handle * HALF),
            size(handle, handle),
        );
        window.paint_quad(quad(
            bounds,
            Pixels::default(),
            theme.ui.background,
            theme.stroke.board_guide,
            theme.ui.board_selection,
            BorderStyle::default(),
        ));
    }
}

/// La letra de un nodo al unir con el teclado, en su esquina.
pub fn paint_hint(
    letters: &str,
    outline: Rect,
    frame: &Frame<'_>,
    window: &mut Window,
    cx: &mut App,
) {
    let theme = frame.theme;
    let padding = theme.layout.board_hint_padding;
    let size_px = theme.ui_small_font_size;
    let run = TextRun {
        len: letters.len(),
        font: font(theme.font_family.clone()),
        color: theme.ui.board_hint_text,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let shaped = window
        .text_system()
        .shape_line(letters.to_owned().into(), size_px, &[run], None);
    let at = frame.on_screen(Point::new(outline.x, outline.y));
    let line_height = theme.ui_small_font_size * theme.text.ui_sm.line_height;
    let bounds = Bounds::new(at, size(shaped.width() + padding + padding, line_height));
    window.paint_quad(quad(
        bounds,
        theme.radius.sm,
        theme.ui.board_hint_background,
        Pixels::default(),
        Hsla::default(),
        BorderStyle::default(),
    ));
    let origin = point(at.x + padding, at.y);
    if let Err(error) = shaped.paint(
        origin,
        line_height,
        gpui_kit::TextAlign::Left,
        None,
        window,
        cx,
    ) {
        log::error!("no se pudo pintar la letra de un nodo: {error:#}");
    }
}

pub fn paint_plan(plan: &Plan<'_>, frame: &Frame<'_>, window: &mut Window, cx: &mut App) {
    for &(element, look) in &plan.elements {
        match look {
            Look::Mesh => paint_meshes(element, frame, window),
            Look::Quad => paint_box(element, frame, window),
        }
    }
    for label in &plan.labels {
        label::paint_label(label, frame, window, cx);
    }
}

fn paint_meshes(element: &ElementScene, frame: &Frame<'_>, window: &mut Window) {
    for piece in &element.pieces {
        paint_mesh(&piece.mesh, piece.role.color(frame.theme), frame, window);
    }
}

fn paint_mesh(mesh: &Mesh, color: Hsla, frame: &Frame<'_>, window: &mut Window) {
    let vertex = |index: u32| {
        mesh.vertices
            .get(index as usize)
            .map(|v| frame.on_screen(Point::new(v.x, v.y)))
    };
    let Some(first) = mesh.indices.first().and_then(|&index| vertex(index)) else {
        return;
    };
    let mut path = Path::new(first);
    // Las coordenadas de la curva de un triángulo lleno.
    let solid = (point(0.0, 1.0), point(0.0, 1.0), point(0.0, 1.0));
    for triangle in mesh.indices.as_chunks::<3>().0 {
        if let [Some(a), Some(b), Some(c)] = triangle.map(vertex) {
            path.push_triangle((a, b, c), solid);
        }
    }
    window.paint_path(path, color);
}

/// Un nodo de lejos: su caja con el relleno y el borde de su malla. Lo que no
/// tiene malla, como un texto suelto, de lejos no pinta nada.
fn paint_box(element: &ElementScene, frame: &Frame<'_>, window: &mut Window) {
    let theme = frame.theme;
    let fill = element.fill_role().map(|role| role.color(theme));
    let line = element.line_role().map(|role| role.color(theme));
    if fill.is_none() && line.is_none() {
        return;
    }
    window.paint_quad(quad(
        frame.bounds_of(element.outline),
        theme.radius.lg * frame.camera.zoom,
        fill.unwrap_or_default(),
        theme.stroke.border,
        line.unwrap_or_default(),
        BorderStyle::default(),
    ));
}

/// El borde del nodo elegido, con el mismo grosor a cualquier zoom.
pub fn paint_selection(outline: Rect, frame: &Frame<'_>, window: &mut Window) {
    let theme = frame.theme;
    window.paint_quad(quad(
        frame.bounds_of(outline),
        theme.radius.lg * frame.camera.zoom,
        Hsla::default(),
        theme.stroke.board_selection,
        theme.ui.board_selection,
        BorderStyle::default(),
    ));
}

/// El fondo de la etiqueta de una flecha, de `width` y centrado en `text_box`:
/// la línea no le pasa por encima al texto.
pub fn fill_behind(
    text_box: Bounds<Pixels>,
    width: Pixels,
    frame: &Frame<'_>,
    window: &mut Window,
) {
    let theme = frame.theme;
    let padded = width + theme.layout.board_padding * frame.camera.zoom;
    let left = text_box.origin.x + (text_box.size.width - padded) * HALF;
    let background = Bounds::new(
        point(left, text_box.origin.y),
        size(padded, text_box.size.height),
    );
    window.paint_quad(fill(background, theme.ui.board_label_background));
}
