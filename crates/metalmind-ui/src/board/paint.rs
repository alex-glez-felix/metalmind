//! Pinta un plan: cada malla, ya teselada en coordenadas de la pizarra, solo
//! pasa por la cámara.

use gpui_kit::{
    App, BorderStyle, Bounds, Hsla, Path, Pixels, Point as ScreenPoint, Window, fill, point, px,
    quad, size,
};
use metalmind_board::{Point, Rect};

use super::camera::Camera;
use super::label;
use super::plan::{Look, Plan};
use super::scene::ElementScene;
use super::shapes::Mesh;
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
