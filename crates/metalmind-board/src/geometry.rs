//! Las coordenadas de la pizarra: siluetas de los nodos, dónde toca una flecha
//! su borde y qué nodo cae bajo un punto.

use crate::board::Board;
use crate::edge::Side;
use crate::node::{Node, NodeKind, Shape};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    #[must_use]
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub(crate) fn offset(self, direction: Self, factor: f32) -> Self {
        Self::new(self.x + direction.x * factor, self.y + direction.y * factor)
    }

    pub(crate) fn distance(self, other: Self) -> f32 {
        (self.x - other.x).hypot(self.y - other.y)
    }

    pub(crate) fn midpoint(self, other: Self) -> Self {
        Self::new(
            f32::midpoint(self.x, other.x),
            f32::midpoint(self.y, other.y),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    /// Si `p` cae dentro de la silueta `shape` inscrita en el rectángulo.
    #[must_use]
    pub fn contains(&self, shape: Shape, p: Point) -> bool {
        let c = self.center();
        let (dx, dy) = ((p.x - c.x) / (self.w / 2.0), (p.y - c.y) / (self.h / 2.0));
        match shape {
            Shape::Ellipse => dx * dx + dy * dy <= 1.0,
            Shape::Diamond => dx.abs() + dy.abs() <= 1.0,
            Shape::Rectangle | Shape::None => dx.abs() <= 1.0 && dy.abs() <= 1.0,
        }
    }

    #[must_use]
    pub fn center(&self) -> Point {
        Point::new(self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    #[must_use]
    pub fn intersects(&self, other: &Self) -> bool {
        self.x < other.x + other.w
            && other.x < self.x + self.w
            && self.y < other.y + other.h
            && other.y < self.y + self.h
    }

    /// El rectángulo más chico que contiene a los dos.
    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        let (x, y) = (self.x.min(other.x), self.y.min(other.y));
        Self {
            x,
            y,
            w: (self.x + self.w).max(other.x + other.w) - x,
            h: (self.y + self.h).max(other.y + other.h) - y,
        }
    }

    pub(crate) fn side_point(&self, side: Side) -> Point {
        let c = self.center();
        match side {
            Side::Top => Point::new(c.x, self.y),
            Side::Bottom => Point::new(c.x, self.y + self.h),
            Side::Left => Point::new(self.x, c.y),
            Side::Right => Point::new(self.x + self.w, c.y),
        }
    }

    /// El lado que mira hacia `to`, según la proporción de las dos cajas.
    pub(crate) fn facing_side(&self, to: &Self) -> Side {
        let (a, b) = (self.center(), to.center());
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let span_x = f32::midpoint(self.w, to.w).max(1.0);
        let span_y = f32::midpoint(self.h, to.h).max(1.0);
        if (dx / span_x).abs() >= (dy / span_y).abs() {
            if dx >= 0.0 { Side::Right } else { Side::Left }
        } else if dy >= 0.0 {
            Side::Bottom
        } else {
            Side::Top
        }
    }

    /// Donde el rayo del centro hacia `target` cruza la silueta.
    pub(crate) fn boundary_toward(&self, shape: Shape, target: Point) -> Point {
        let c = self.center();
        let (dx, dy) = (target.x - c.x, target.y - c.y);
        let (rx, ry) = (self.w / 2.0, self.h / 2.0);
        if dx.abs() < f32::EPSILON && dy.abs() < f32::EPSILON {
            return c;
        }
        let t = match shape {
            Shape::Ellipse => 1.0 / ((dx / rx).powi(2) + (dy / ry).powi(2)).sqrt(),
            Shape::Diamond => 1.0 / (dx.abs() / rx + dy.abs() / ry),
            Shape::Rectangle | Shape::None => 1.0 / (dx.abs() / rx).max(dy.abs() / ry),
        };
        Point::new(c.x + dx * t, c.y + dy * t)
    }
}

impl Node {
    /// La caja del nodo. Mide al menos 1×1: así ninguna cuenta de la silueta
    /// divide por cero, aunque el archivo diga `"width":0`.
    #[must_use]
    #[expect(
        clippy::cast_precision_loss,
        reason = "las coordenadas de una pizarra caben en f32"
    )]
    pub fn rect(&self) -> Rect {
        Rect {
            x: self.x as f32,
            y: self.y as f32,
            w: (self.width as f32).max(1.0),
            h: (self.height as f32).max(1.0),
        }
    }

    /// La silueta donde se pega una flecha y donde acierta un clic.
    #[must_use]
    pub fn outline(&self) -> Shape {
        match &self.kind {
            NodeKind::Text { shape, .. } if *shape != Shape::None => *shape,
            _ => Shape::Rectangle,
        }
    }
}

impl Board {
    /// El nodo de más arriba bajo `point`, por su silueta: la esquina vacía de
    /// una elipse no la toca. Los grupos son fondo y no cuentan.
    #[must_use]
    pub fn hit(&self, point: Point) -> Option<&Node> {
        self.nodes.iter().rev().find(|node| {
            !matches!(node.kind, NodeKind::Group { .. })
                && node.rect().contains(node.outline(), point)
        })
    }
}

/// El punto de la curva de Bézier `curve` en `t` ∈ [0, 1].
#[must_use]
pub fn cubic_at(curve: [Point; 4], t: f32) -> Point {
    let u = 1.0 - t;
    let weights = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
    let (x, y) = curve
        .iter()
        .zip(weights)
        .fold((0.0, 0.0), |(x, y), (p, w)| (x + w * p.x, y + w * p.y));
    Point::new(x, y)
}

/// Los tres vértices de una punta de flecha en `tip`, llegando desde `from`.
#[must_use]
pub fn arrowhead(tip: Point, from: Point, size: f32) -> [Point; 3] {
    let length = tip.distance(from).max(f32::EPSILON);
    let (ux, uy) = ((tip.x - from.x) / length, (tip.y - from.y) / length);
    let base = Point::new(tip.x - ux * size, tip.y - uy * size);
    let half = size * 0.45;
    [
        tip,
        Point::new(base.x - uy * half, base.y + ux * half),
        Point::new(base.x + uy * half, base.y - ux * half),
    ]
}

#[cfg(test)]
#[path = "geometry_tests.rs"]
mod tests;
