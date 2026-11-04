//! Por dónde pasa una flecha. Se calcula siempre de los nodos que une, así
//! que mover una caja re-enruta sus flechas sin tocarlas en el archivo.

use std::collections::HashMap;

use crate::board::Board;
use crate::edge::{Edge, Route, Side};
use crate::geometry::{Point, Rect, cubic_at};
use crate::node::{Node, Shape};

const ELBOW_STUB: f32 = 24.0;
const LOOP_REACH: f32 = 48.0;
const LOOP_NEAR_CORNER: f32 = 24.0;

#[derive(Clone, Debug, PartialEq)]
pub enum EdgePath {
    Polyline(Vec<Point>),
    Cubic([Point; 4]),
}

#[derive(Clone, Debug, PartialEq)]
pub struct EdgeGeometry {
    pub path: EdgePath,
    /// De dónde viene la línea al llegar a cada punta: orienta la flecha.
    pub start_from: Point,
    pub end_from: Point,
    pub label_at: Point,
}

impl EdgeGeometry {
    #[must_use]
    pub fn start(&self) -> Point {
        match &self.path {
            EdgePath::Polyline(points) => points.first().copied().unwrap_or(self.label_at),
            EdgePath::Cubic(curve) => curve[0],
        }
    }

    #[must_use]
    pub fn end(&self) -> Point {
        match &self.path {
            EdgePath::Polyline(points) => points.last().copied().unwrap_or(self.label_at),
            EdgePath::Cubic(curve) => curve[3],
        }
    }
}

impl Board {
    /// Cada flecha con su recorrido, sin buscar los dos nodos por cada una.
    pub fn edge_geometries(&self) -> impl Iterator<Item = (&Edge, EdgeGeometry)> {
        let nodes: HashMap<&str, &Node> = self.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
        self.edges.iter().filter_map(move |edge| {
            let from = nodes.get(edge.from.node.as_str())?;
            let to = nodes.get(edge.to.node.as_str())?;
            Some((edge, edge.geometry(from, to)))
        })
    }
}

impl Edge {
    /// El recorrido entre `from` y `to`, los nodos que la flecha nombra.
    #[must_use]
    pub fn geometry(&self, from: &Node, to: &Node) -> EdgeGeometry {
        let (a, b) = (from.rect(), to.rect());
        if from.id == to.id {
            return self_loop(&a, from.outline());
        }
        let from_side = self.from.side.unwrap_or_else(|| a.facing_side(&b));
        let to_side = self.to.side.unwrap_or_else(|| b.facing_side(&a));
        match self.route {
            Route::Straight => {
                let start = self.from.side.map_or_else(
                    || a.boundary_toward(from.outline(), b.center()),
                    |side| a.side_point(side),
                );
                let end = self.to.side.map_or_else(
                    || b.boundary_toward(to.outline(), a.center()),
                    |side| b.side_point(side),
                );
                EdgeGeometry {
                    path: EdgePath::Polyline(vec![start, end]),
                    start_from: end,
                    end_from: start,
                    label_at: start.midpoint(end),
                }
            }
            Route::Curve => curve(
                a.side_point(from_side),
                from_side,
                b.side_point(to_side),
                to_side,
            ),
            Route::Elbow => elbow(
                a.side_point(from_side),
                from_side,
                b.side_point(to_side),
                to_side,
            ),
        }
    }
}

fn normal(side: Side) -> Point {
    match side {
        Side::Top => Point::new(0.0, -1.0),
        Side::Bottom => Point::new(0.0, 1.0),
        Side::Left => Point::new(-1.0, 0.0),
        Side::Right => Point::new(1.0, 0.0),
    }
}

fn is_horizontal(side: Side) -> bool {
    matches!(side, Side::Left | Side::Right)
}

fn curve(start: Point, from_side: Side, end: Point, to_side: Side) -> EdgeGeometry {
    let reach = (start.distance(end) * 0.5).clamp(20.0, 150.0);
    let c1 = start.offset(normal(from_side), reach);
    let c2 = end.offset(normal(to_side), reach);
    let curve = [start, c1, c2, end];
    EdgeGeometry {
        path: EdgePath::Cubic(curve),
        start_from: c1,
        end_from: c2,
        label_at: cubic_at(curve, 0.5),
    }
}

/// Un lazo en la esquina de arriba a la derecha, por fuera de la silueta. Sus
/// extremos se apoyan cerca de esa esquina, sea cual sea el tamaño de la caja.
fn self_loop(r: &Rect, shape: Shape) -> EdgeGeometry {
    let near = LOOP_NEAR_CORNER.min(r.w / 3.0).min(r.h / 3.0);
    let start = r.boundary_toward(shape, Point::new(r.x + r.w, r.y + near));
    let end = r.boundary_toward(shape, Point::new(r.x + r.w - near, r.y));
    let c1 = start.offset(normal(Side::Right), LOOP_REACH);
    let c2 = end.offset(normal(Side::Top), LOOP_REACH);
    let curve = [start, c1, c2, end];
    EdgeGeometry {
        path: EdgePath::Cubic(curve),
        start_from: c1,
        end_from: c2,
        label_at: cubic_at(curve, 0.5),
    }
}

fn elbow(start: Point, from_side: Side, end: Point, to_side: Side) -> EdgeGeometry {
    let s1 = start.offset(normal(from_side), ELBOW_STUB);
    let e1 = end.offset(normal(to_side), ELBOW_STUB);
    let mut points = vec![start, s1];
    match (is_horizontal(from_side), is_horizontal(to_side)) {
        (true, true) => {
            let mid = f32::midpoint(s1.x, e1.x);
            points.extend([Point::new(mid, s1.y), Point::new(mid, e1.y)]);
        }
        (false, false) => {
            let mid = f32::midpoint(s1.y, e1.y);
            points.extend([Point::new(s1.x, mid), Point::new(e1.x, mid)]);
        }
        (true, false) => points.push(Point::new(e1.x, s1.y)),
        (false, true) => points.push(Point::new(s1.x, e1.y)),
    }
    points.extend([e1, end]);
    points.dedup_by(|a, b| a.distance(*b) < 0.5);
    EdgeGeometry {
        start_from: points.get(1).copied().unwrap_or(end),
        end_from: points.iter().rev().nth(1).copied().unwrap_or(start),
        label_at: polyline_middle(&points),
        path: EdgePath::Polyline(points),
    }
}

fn polyline_middle(points: &[Point]) -> Point {
    let total: f32 = points.windows(2).map(|w| w[0].distance(w[1])).sum();
    let mut left = total / 2.0;
    for w in points.windows(2) {
        let length = w[0].distance(w[1]);
        if length >= left && length > 0.0 {
            let t = left / length;
            return Point::new(
                w[0].x + (w[1].x - w[0].x) * t,
                w[0].y + (w[1].y - w[0].y) * t,
            );
        }
        left -= length;
    }
    points.first().copied().unwrap_or(Point::new(0.0, 0.0))
}

#[cfg(test)]
#[path = "route_tests.rs"]
mod tests;
