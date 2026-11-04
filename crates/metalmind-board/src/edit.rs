//! Lo que una edición le hace a la pizarra: crear, unir, mover, redimensionar,
//! escribir y borrar. Sin historial ni archivo: de eso se encarga quien edita
//! el documento, con las líneas que cambian en [`crate::line_edits`].

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Map;

use crate::board::Board;
use crate::edge::{Edge, EdgeEnd, Route, Side, Tip};
use crate::geometry::{Point, Rect};
use crate::node::{Node, NodeKind, Shape};

/// La rejilla de la pizarra, en unidades: 09 §4.4.
pub const GRID: i32 = 10;
/// Lo que mide una forma creada con un clic o con `enter`.
pub const DEFAULT_SHAPE: (i32, i32) = (200, 80);
pub const DEFAULT_TEXT: (i32, i32) = (200, 40);
/// Lo que separa una caja nueva de la caja desde la que se creó.
const LINKED_GAP: i32 = 100;

/// La caja de un nodo, en unidades de la pizarra.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Una esquina de un nodo, por donde se redimensiona.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// El múltiplo de [`GRID`] más cercano.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "las coordenadas de una pizarra caben en i32 y en f32"
)]
pub fn snap(value: f32) -> i32 {
    (value / GRID as f32).round() as i32 * GRID
}

impl Frame {
    /// Una caja de `size` centrada en `center`, con su esquina en la rejilla.
    #[must_use]
    #[expect(
        clippy::cast_precision_loss,
        reason = "las medidas de una pizarra caben en f32"
    )]
    pub fn centered_at(center: Point, (width, height): (i32, i32)) -> Self {
        Self {
            x: snap(center.x - width as f32 / 2.0),
            y: snap(center.y - height as f32 / 2.0),
            width,
            height,
        }
    }

    /// La caja entre dos puntos en cualquier orden, sobre la rejilla y nunca
    /// menor que una celda.
    #[must_use]
    pub fn between(a: Point, b: Point) -> Self {
        let (left, right) = (snap(a.x.min(b.x)), snap(a.x.max(b.x)));
        let (top, bottom) = (snap(a.y.min(b.y)), snap(a.y.max(b.y)));
        Self {
            x: left,
            y: top,
            width: (right - left).max(GRID),
            height: (bottom - top).max(GRID),
        }
    }

    #[must_use]
    #[expect(
        clippy::cast_precision_loss,
        reason = "las coordenadas de una pizarra caben en f32"
    )]
    pub fn rect(self) -> Rect {
        Rect {
            x: self.x as f32,
            y: self.y as f32,
            w: self.width as f32,
            h: self.height as f32,
        }
    }

    /// Esta caja con `corner` llevada a `to`: la esquina opuesta no se mueve.
    #[must_use]
    pub fn dragged(self, corner: Corner, to: Point) -> Self {
        let (right, bottom) = (self.x + self.width, self.y + self.height);
        let (to_x, to_y) = (snap(to.x), snap(to.y));
        let (x, width) = match corner {
            Corner::TopLeft | Corner::BottomLeft => {
                let x = to_x.min(right - GRID);
                (x, right - x)
            }
            Corner::TopRight | Corner::BottomRight => (self.x, (to_x - self.x).max(GRID)),
        };
        let (y, height) = match corner {
            Corner::TopLeft | Corner::TopRight => {
                let y = to_y.min(bottom - GRID);
                (y, bottom - y)
            }
            Corner::BottomLeft | Corner::BottomRight => (self.y, (to_y - self.y).max(GRID)),
        };
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

impl Node {
    #[must_use]
    pub fn frame(&self) -> Frame {
        Frame {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
        }
    }

    pub fn set_frame(&mut self, frame: Frame) {
        self.x = frame.x;
        self.y = frame.y;
        self.width = frame.width;
        self.height = frame.height;
    }

    /// El texto que se escribe en él: solo una caja o un texto suelto lo tienen.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        match &self.kind {
            NodeKind::Text { text, .. } => Some(text),
            _ => None,
        }
    }
}

impl Board {
    /// Un id que no usa ningún elemento, como los de 16 cifras de Obsidian.
    #[must_use]
    pub fn fresh_id(&self) -> String {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let clock = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_secs() ^ u64::from(since.subsec_nanos()));
        loop {
            let serial = NEXT.fetch_add(1, Ordering::Relaxed);
            let mixed = (clock ^ serial.rotate_left(32)).wrapping_mul(0x9E37_79B9_7F4A_7C15);
            let id = format!("{mixed:016x}");
            if !self.has_element(&id) {
                return id;
            }
        }
    }

    fn has_element(&self, id: &str) -> bool {
        self.node(id).is_some() || self.edges.iter().any(|edge| edge.id == id)
    }

    /// Agrega encima de todo una caja, una forma o un texto suelto. Da su id.
    pub fn add_text(&mut self, frame: Frame, shape: Shape, text: &str) -> String {
        let id = self.fresh_id();
        self.nodes.push(Node {
            id: id.clone(),
            x: frame.x,
            y: frame.y,
            width: frame.width,
            height: frame.height,
            color: None,
            kind: NodeKind::Text {
                text: text.to_owned(),
                shape,
            },
            extra: Map::new(),
        });
        id
    }

    /// Una flecha de `from` a `to`. Da su id, o `None` si falta uno de los dos.
    pub fn connect(&mut self, from: &str, to: &str) -> Option<String> {
        self.node(from)?;
        self.node(to)?;
        let id = self.fresh_id();
        let end = |node: &str, tip| EdgeEnd {
            node: node.to_owned(),
            side: None,
            tip,
        };
        self.edges.push(Edge {
            id: id.clone(),
            from: end(from, Tip::None),
            to: end(to, Tip::Arrow),
            route: Route::Curve,
            color: None,
            label: None,
            extra: Map::new(),
        });
        Some(id)
    }

    /// Una caja nueva hacia `side` de `from`, unida a ella por una flecha, en
    /// el primer lugar libre. Da el id de la caja.
    pub fn add_linked_box(&mut self, from: &str, side: Side) -> Option<String> {
        let source = self.node(from)?.frame();
        let frame = self.free_frame_toward(source, side);
        let id = self.add_text(frame, Shape::Rectangle, "");
        self.connect(from, &id)?;
        Some(id)
    }

    fn free_frame_toward(&self, source: Frame, side: Side) -> Frame {
        let (step_x, step_y) = match side {
            Side::Left => (-(source.width + LINKED_GAP), 0),
            Side::Right => (source.width + LINKED_GAP, 0),
            Side::Top => (0, -(source.height + LINKED_GAP)),
            Side::Bottom => (0, source.height + LINKED_GAP),
        };
        let mut frame = Frame {
            x: source.x + step_x,
            y: source.y + step_y,
            ..source
        };
        while self.is_taken(frame) {
            frame.x += step_x;
            frame.y += step_y;
        }
        frame
    }

    fn is_taken(&self, frame: Frame) -> bool {
        let rect = frame.rect();
        self.nodes
            .iter()
            .filter(|node| !matches!(node.kind, NodeKind::Group { .. }))
            .any(|node| node.rect().intersects(&rect))
    }

    /// `frame`, corrida en diagonal hasta que no quede justo sobre otro nodo.
    #[must_use]
    pub fn unstacked(&self, mut frame: Frame) -> Frame {
        while self
            .nodes
            .iter()
            .any(|node| (node.x, node.y) == (frame.x, frame.y))
        {
            frame.x += 2 * GRID;
            frame.y += 2 * GRID;
        }
        frame
    }

    /// Mueve los nodos de `ids`; las flechas se calculan de ellos.
    pub fn move_nodes(&mut self, ids: &[String], dx: i32, dy: i32) {
        for node in self.nodes.iter_mut().filter(|node| ids.contains(&node.id)) {
            node.x += dx;
            node.y += dy;
        }
    }

    /// Cambia el texto de una caja, o la etiqueta de una flecha (vacía, no
    /// tiene). `false` si `id` no tiene texto.
    pub fn set_text(&mut self, id: &str, new: &str) -> bool {
        if let Some(edge) = self.edges.iter_mut().find(|edge| edge.id == id) {
            edge.label = (!new.is_empty()).then(|| new.to_owned());
            return true;
        }
        match self.nodes.iter_mut().find(|node| node.id == id) {
            Some(Node {
                kind: NodeKind::Text { text, .. },
                ..
            }) => {
                new.clone_into(text);
                true
            }
            _ => false,
        }
    }

    /// Borra `ids`, y con cada nodo las flechas que le llegan: una flecha
    /// siempre une dos nodos (09 §2.3).
    pub fn remove(&mut self, ids: &[String]) {
        self.nodes.retain(|node| !ids.contains(&node.id));
        let alive: Vec<&str> = self.nodes.iter().map(|node| node.id.as_str()).collect();
        self.edges.retain(|edge| {
            !ids.contains(&edge.id)
                && alive.contains(&edge.from.node.as_str())
                && alive.contains(&edge.to.node.as_str())
        });
    }
}

#[cfg(test)]
#[path = "edit_tests.rs"]
mod tests;
