//! Qué se pinta de la escena en un frame. Sin GPUI, para probarlo: el nivel de
//! detalle y el tope de triángulos son lo que evita que una pizarra grande
//! pase del búfer de instancias de 128 MB de GPUI (09 §3.3), que no pinta el
//! frame entero si se pasa.

use metalmind_board::Rect;

use super::camera::{AreaSize, Camera};
use super::scene::{ElementScene, Label, Scene};

/// Por debajo de este zoom un nodo es un quad de GPUI, su caja con su relleno y
/// su borde, y no su malla: de lejos no se distingue.
pub const DETAIL_ZOOM: f32 = 0.35;

/// Cada triángulo de un path ocupa ~190 bytes en el búfer de GPUI (medido en
/// 09 §3.3: ~700 k triángulos llegan al tope). Con esto queda margen para los
/// quads, el texto y el resto de la ventana.
const MAX_TRIANGLES: usize = 350_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    Mesh,
    Quad,
}

#[derive(Debug, Default)]
pub struct Plan<'a> {
    /// En orden: lo primero queda abajo. Una flecha que no cupo no está.
    pub elements: Vec<(&'a ElementScene, Look)>,
    pub labels: Vec<&'a Label>,
    pub triangles: usize,
}

#[must_use]
pub fn plan(scene: &Scene, camera: Camera, area: AreaSize) -> Plan<'_> {
    let shown = camera.visible(area);
    let far = camera.zoom < DETAIL_ZOOM;
    let mut plan = Plan::default();
    for group in visible(&scene.groups, shown) {
        plan.push_box(group, far);
    }
    for edge in visible(&scene.edges, shown) {
        plan.push_edge(edge);
    }
    for node in visible(&scene.nodes, shown) {
        plan.push_box(node, far);
    }
    let labelled = visible(&scene.groups, shown)
        .chain(visible(&scene.nodes, shown))
        .chain(visible(&scene.edges, shown));
    plan.labels = labelled.flat_map(|element| &element.labels).collect();
    plan
}

fn visible(elements: &[ElementScene], shown: Rect) -> impl Iterator<Item = &ElementScene> {
    elements
        .iter()
        .filter(move |element| element.bounds.intersects(&shown))
}

impl<'a> Plan<'a> {
    fn fits(&self, element: &ElementScene) -> bool {
        self.triangles + element.triangles() <= MAX_TRIANGLES
    }

    /// Un nodo siempre se ve: si su malla no cabe, va como quad.
    fn push_box(&mut self, element: &'a ElementScene, far: bool) {
        if far || !self.fits(element) {
            self.elements.push((element, Look::Quad));
        } else {
            self.triangles += element.triangles();
            self.elements.push((element, Look::Mesh));
        }
    }

    fn push_edge(&mut self, element: &'a ElementScene) {
        if self.fits(element) {
            self.triangles += element.triangles();
            self.elements.push((element, Look::Mesh));
        }
    }
}

#[cfg(test)]
#[path = "plan_tests.rs"]
mod tests;
