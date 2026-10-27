//! Por dónde pasa una flecha. Se calcula siempre de los nodos que une, así
//! que mover una caja re-enruta sus flechas sin tocarlas en el archivo.

use crate::formato::{Board, Edge, Node, NodeKind, Route, Shape, Side};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct P {
    pub x: f32,
    pub y: f32,
}

impl P {
    #[must_use]
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    fn add(self, other: Self, factor: f32) -> Self {
        Self::new(self.x + other.x * factor, self.y + other.y * factor)
    }

    fn distance(self, other: Self) -> f32 {
        (self.x - other.x).hypot(self.y - other.y)
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
    /// Un nodo mide al menos 1×1: así ninguna cuenta de la silueta divide
    /// por cero, aunque el archivo diga `"width":0`.
    #[must_use]
    pub fn of(node: &Node) -> Self {
        #[expect(
            clippy::cast_precision_loss,
            reason = "las coordenadas de una pizarra caben en f32"
        )]
        Self {
            x: node.x as f32,
            y: node.y as f32,
            w: (node.width as f32).max(1.0),
            h: (node.height as f32).max(1.0),
        }
    }

    /// Si `p` cae dentro de la silueta `shape` inscrita en el rectángulo.
    #[must_use]
    pub fn contains(&self, shape: Shape, p: P) -> bool {
        let c = self.center();
        let (dx, dy) = ((p.x - c.x) / (self.w / 2.0), (p.y - c.y) / (self.h / 2.0));
        match shape {
            Shape::Ellipse => dx * dx + dy * dy <= 1.0,
            Shape::Diamond => dx.abs() + dy.abs() <= 1.0,
            Shape::Rectangle | Shape::None => dx.abs() <= 1.0 && dy.abs() <= 1.0,
        }
    }

    #[must_use]
    pub fn center(&self) -> P {
        P::new(self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    #[must_use]
    pub fn intersects(&self, other: &Self) -> bool {
        self.x < other.x + other.w
            && other.x < self.x + self.w
            && self.y < other.y + other.h
            && other.y < self.y + self.h
    }

    fn side_point(&self, side: Side) -> P {
        let c = self.center();
        match side {
            Side::Top => P::new(c.x, self.y),
            Side::Bottom => P::new(c.x, self.y + self.h),
            Side::Left => P::new(self.x, c.y),
            Side::Right => P::new(self.x + self.w, c.y),
        }
    }
}

/// La silueta de un nodo, que es donde se pega una flecha.
#[must_use]
pub fn outline(node: &Node) -> Shape {
    match &node.kind {
        NodeKind::Text {
            shape: Shape::None, ..
        }
        | NodeKind::Stroke { .. } => Shape::Rectangle,
        NodeKind::Text { shape, .. } => *shape,
        _ => Shape::Rectangle,
    }
}

fn normal(side: Side) -> P {
    match side {
        Side::Top => P::new(0.0, -1.0),
        Side::Bottom => P::new(0.0, 1.0),
        Side::Left => P::new(-1.0, 0.0),
        Side::Right => P::new(1.0, 0.0),
    }
}

fn horizontal(side: Side) -> bool {
    matches!(side, Side::Left | Side::Right)
}

/// El lado de `from` que mira hacia `to`, según la proporción de las cajas.
#[must_use]
pub fn facing_side(from: &Rect, to: &Rect) -> Side {
    let (a, b) = (from.center(), to.center());
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let (span_x, span_y) = (f32::midpoint(from.w, to.w), f32::midpoint(from.h, to.h));
    if (dx / span_x.max(1.0)).abs() >= (dy / span_y.max(1.0)).abs() {
        if dx >= 0.0 { Side::Right } else { Side::Left }
    } else if dy >= 0.0 {
        Side::Bottom
    } else {
        Side::Top
    }
}

/// Donde el rayo del centro hacia `target` cruza la silueta.
#[must_use]
pub fn boundary_toward(rect: &Rect, shape: Shape, target: P) -> P {
    let c = rect.center();
    let (dx, dy) = (target.x - c.x, target.y - c.y);
    let (rx, ry) = (rect.w / 2.0, rect.h / 2.0);
    if dx.abs() < f32::EPSILON && dy.abs() < f32::EPSILON {
        return c;
    }
    let t = match shape {
        Shape::Ellipse => 1.0 / ((dx / rx).powi(2) + (dy / ry).powi(2)).sqrt(),
        Shape::Diamond => 1.0 / (dx.abs() / rx + dy.abs() / ry),
        Shape::Rectangle | Shape::None => 1.0 / (dx.abs() / rx).max(dy.abs() / ry),
    };
    P::new(c.x + dx * t, c.y + dy * t)
}

#[derive(Clone, Debug, PartialEq)]
pub enum EdgePath {
    Polyline(Vec<P>),
    Cubic([P; 4]),
}

#[derive(Clone, Debug, PartialEq)]
pub struct EdgeGeometry {
    pub path: EdgePath,
    /// Dirección de llegada a cada punta: de dónde viene la línea.
    pub start_from: P,
    pub end_from: P,
    pub label_at: P,
}

impl EdgeGeometry {
    #[must_use]
    pub fn start(&self) -> P {
        match &self.path {
            EdgePath::Polyline(points) => points.first().copied().unwrap_or(self.label_at),
            EdgePath::Cubic(c) => c[0],
        }
    }

    #[must_use]
    pub fn end(&self) -> P {
        match &self.path {
            EdgePath::Polyline(points) => points.last().copied().unwrap_or(self.label_at),
            EdgePath::Cubic(c) => c[3],
        }
    }
}

const ELBOW_STUB: f32 = 24.0;

/// `None` si la flecha apunta a un nodo que no está en `board`.
#[must_use]
pub fn route(board: &Board, edge: &Edge) -> Option<EdgeGeometry> {
    let (from, to) = (board.node(&edge.from.node)?, board.node(&edge.to.node)?);
    Some(route_between(from, to, edge))
}

#[must_use]
pub fn route_between(from: &Node, to: &Node, edge: &Edge) -> EdgeGeometry {
    let (a, b) = (Rect::of(from), Rect::of(to));
    if from.id == to.id {
        return self_loop(&a);
    }
    let from_side = edge.from.side.unwrap_or_else(|| facing_side(&a, &b));
    let to_side = edge.to.side.unwrap_or_else(|| facing_side(&b, &a));
    match edge.route {
        Route::Straight => {
            let start = match edge.from.side {
                Some(side) => a.side_point(side),
                None => boundary_toward(&a, outline(from), b.center()),
            };
            let end = match edge.to.side {
                Some(side) => b.side_point(side),
                None => boundary_toward(&b, outline(to), a.center()),
            };
            EdgeGeometry {
                path: EdgePath::Polyline(vec![start, end]),
                start_from: end,
                end_from: start,
                label_at: P::new(f32::midpoint(start.x, end.x), f32::midpoint(start.y, end.y)),
            }
        }
        Route::Curve => {
            let (p0, p3) = (a.side_point(from_side), b.side_point(to_side));
            let reach = (p0.distance(p3) * 0.5).clamp(20.0, 150.0);
            let c1 = p0.add(normal(from_side), reach);
            let c2 = p3.add(normal(to_side), reach);
            EdgeGeometry {
                path: EdgePath::Cubic([p0, c1, c2, p3]),
                start_from: c1,
                end_from: c2,
                label_at: cubic_at([p0, c1, c2, p3], 0.5),
            }
        }
        Route::Elbow => elbow(
            a.side_point(from_side),
            from_side,
            b.side_point(to_side),
            to_side,
        ),
    }
}

/// Una flecha de un nodo a sí mismo: sale por la derecha y vuelve por
/// arriba, por fuera de la caja, como en Obsidian.
fn self_loop(r: &Rect) -> EdgeGeometry {
    let reach = (r.w.min(r.h) * 0.6).clamp(40.0, 90.0);
    let start = P::new(r.x + r.w, r.y + r.h * 0.35);
    let end = P::new(r.x + r.w * 0.65, r.y);
    let c1 = start.add(normal(Side::Right), reach);
    let c2 = end.add(normal(Side::Top), reach);
    let curve = [start, c1, c2, end];
    EdgeGeometry {
        path: EdgePath::Cubic(curve),
        start_from: c1,
        end_from: c2,
        label_at: cubic_at(curve, 0.5),
    }
}

fn elbow(start: P, from_side: Side, end: P, to_side: Side) -> EdgeGeometry {
    let s1 = start.add(normal(from_side), ELBOW_STUB);
    let e1 = end.add(normal(to_side), ELBOW_STUB);
    let mut points = vec![start, s1];
    match (horizontal(from_side), horizontal(to_side)) {
        (true, true) => {
            let mid = f32::midpoint(s1.x, e1.x);
            points.extend([P::new(mid, s1.y), P::new(mid, e1.y)]);
        }
        (false, false) => {
            let mid = f32::midpoint(s1.y, e1.y);
            points.extend([P::new(s1.x, mid), P::new(e1.x, mid)]);
        }
        (true, false) => points.push(P::new(e1.x, s1.y)),
        (false, true) => points.push(P::new(s1.x, e1.y)),
    }
    points.extend([e1, end]);
    points.dedup_by(|a, b| a.distance(*b) < 0.5);
    let label_at = polyline_middle(&points);
    let start_from = points.get(1).copied().unwrap_or(end);
    let end_from = points.iter().rev().nth(1).copied().unwrap_or(start);
    EdgeGeometry {
        path: EdgePath::Polyline(points),
        start_from,
        end_from,
        label_at,
    }
}

/// El punto de la curva de Bézier `curve` en `t` ∈ [0, 1].
#[must_use]
pub fn cubic_at(curve: [P; 4], t: f32) -> P {
    let u = 1.0 - t;
    let weights = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
    let (x, y) = curve
        .iter()
        .zip(weights)
        .fold((0.0, 0.0), |(x, y), (p, w)| (x + w * p.x, y + w * p.y));
    P::new(x, y)
}

fn polyline_middle(points: &[P]) -> P {
    let total: f32 = points.windows(2).map(|w| w[0].distance(w[1])).sum();
    let mut left = total / 2.0;
    for w in points.windows(2) {
        let length = w[0].distance(w[1]);
        if length >= left && length > 0.0 {
            let t = left / length;
            return P::new(
                w[0].x + (w[1].x - w[0].x) * t,
                w[0].y + (w[1].y - w[0].y) * t,
            );
        }
        left -= length;
    }
    points.first().copied().unwrap_or(P::new(0.0, 0.0))
}

/// Los tres vértices de una punta de flecha en `tip`, llegando desde `from`.
#[must_use]
pub fn arrowhead(tip: P, from: P, size: f32) -> [P; 3] {
    let length = tip.distance(from).max(f32::EPSILON);
    let (ux, uy) = ((tip.x - from.x) / length, (tip.y - from.y) / length);
    let base = P::new(tip.x - ux * size, tip.y - uy * size);
    let half = size * 0.45;
    [
        tip,
        P::new(base.x - uy * half, base.y + ux * half),
        P::new(base.x + uy * half, base.y - ux * half),
    ]
}

/// El nodo de más arriba bajo `point`, por su silueta: la esquina vacía de
/// una elipse no la toca. Los grupos no cuentan.
#[must_use]
pub fn hit(board: &Board, point: P) -> Option<usize> {
    board.nodes.iter().rposition(|node| {
        !matches!(node.kind, NodeKind::Group { .. })
            && Rect::of(node).contains(outline(node), point)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generar;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    fn close(a: P, b: P) -> bool {
        a.distance(b) < 1e-3
    }

    #[test]
    fn facing_side_looks_at_the_other_box() {
        let a = rect(0.0, 0.0, 100.0, 50.0);
        assert_eq!(
            facing_side(&a, &rect(300.0, 10.0, 100.0, 50.0)),
            Side::Right
        );
        assert_eq!(facing_side(&a, &rect(-300.0, 0.0, 100.0, 50.0)), Side::Left);
        assert_eq!(
            facing_side(&a, &rect(0.0, 200.0, 100.0, 50.0)),
            Side::Bottom
        );
        assert_eq!(facing_side(&a, &rect(20.0, -200.0, 100.0, 50.0)), Side::Top);
    }

    #[test]
    fn boundary_toward_lands_on_each_outline() {
        let r = rect(0.0, 0.0, 200.0, 100.0);
        assert!(close(
            boundary_toward(&r, Shape::Rectangle, P::new(500.0, 50.0)),
            P::new(200.0, 50.0)
        ));
        assert!(close(
            boundary_toward(&r, Shape::Ellipse, P::new(100.0, 400.0)),
            P::new(100.0, 100.0)
        ));
        assert!(close(
            boundary_toward(&r, Shape::Diamond, P::new(-300.0, 50.0)),
            P::new(0.0, 50.0)
        ));
        // En diagonal, el rombo queda más adentro que la elipse y esta que el rectángulo.
        let target = P::new(400.0, 250.0);
        let c = r.center();
        let [rq, el, di] = [Shape::Rectangle, Shape::Ellipse, Shape::Diamond]
            .map(|s| boundary_toward(&r, s, target).distance(c));
        assert!(rq > el && el > di, "{rq} {el} {di}");
        let on_ellipse = boundary_toward(&r, Shape::Ellipse, target);
        let (dx, dy) = ((on_ellipse.x - 100.0) / 100.0, (on_ellipse.y - 50.0) / 50.0);
        assert!((dx * dx + dy * dy - 1.0).abs() < 1e-3);
    }

    #[test]
    fn moving_a_box_reroutes_its_arrows() {
        let mut board = generar::example();
        let edge = board.edges.iter().find(|e| e.id == "e2").cloned().unwrap();
        let before = route(&board, &edge).unwrap();
        // Al poner `vault` a la derecha de `formato`, la flecha sale por el otro lado.
        let vault = board.nodes.iter_mut().find(|n| n.id == "vault").unwrap();
        vault.x += 600;
        vault.y -= 150;
        let after = route(&board, &edge).unwrap();
        assert_ne!(before.start(), after.start());
        let formato = board.node("formato").map(Rect::of).unwrap();
        assert!(
            close(
                after.start(),
                P::new(formato.x + formato.w, formato.center().y)
            ),
            "{:?}",
            after.start()
        );
    }

    #[test]
    fn an_elbow_only_turns_at_right_angles() {
        let board = generar::example();
        let edge = board.edges.iter().find(|e| e.id == "e5").unwrap();
        let EdgePath::Polyline(points) = route(&board, edge).unwrap().path else {
            panic!("un codo es una polilínea");
        };
        for pair in points.windows(2) {
            let (dx, dy) = ((pair[1].x - pair[0].x).abs(), (pair[1].y - pair[0].y).abs());
            assert!(dx < 1e-3 || dy < 1e-3, "{pair:?}");
        }
    }

    #[test]
    fn arrowhead_points_back_along_the_line() {
        let [tip, a, b] = arrowhead(P::new(100.0, 0.0), P::new(0.0, 0.0), 12.0);
        assert!(close(tip, P::new(100.0, 0.0)));
        assert!((a.x - 88.0).abs() < 1e-3 && (b.x - 88.0).abs() < 1e-3);
        assert!((a.y + b.y).abs() < 1e-3);
    }

    #[test]
    fn a_zero_sized_box_still_routes_to_finite_points() {
        let mut board = generar::example();
        let edge = board.edges.iter().find(|e| e.id == "e3").cloned().unwrap();
        let vault = board.nodes.iter_mut().find(|n| n.id == "vault").unwrap();
        (vault.width, vault.height) = (0, 0);
        let g = route(&board, &edge).unwrap();
        assert!(
            g.start().x.is_finite() && g.start().y.is_finite(),
            "{:?}",
            g.start()
        );
    }

    #[test]
    fn clicking_the_empty_corner_of_an_ellipse_misses_it() {
        let board = generar::example();
        let indice = board.node("indice").map(Rect::of).unwrap();
        let center = indice.center();
        let corner = P::new(indice.x + 3.0, indice.y + 3.0);
        assert_eq!(
            hit(&board, center).map(|i| board.nodes[i].id.as_str()),
            Some("indice")
        );
        assert_ne!(
            hit(&board, corner).map(|i| board.nodes[i].id.as_str()),
            Some("indice")
        );
        let decide = board.node("decide").map(Rect::of).unwrap();
        assert_eq!(
            hit(&board, P::new(decide.x + 4.0, decide.y + 4.0)),
            None,
            "esquina del rombo"
        );
    }

    #[test]
    fn an_arrow_to_its_own_node_loops_outside_it() {
        let board = generar::example();
        for route in [Route::Curve, Route::Straight, Route::Elbow] {
            let mut edge = board.edges[0].clone();
            edge.to.node.clone_from(&edge.from.node);
            edge.route = route;
            let g = route_between(
                board.node(&edge.from.node).unwrap(),
                board.node(&edge.to.node).unwrap(),
                &edge,
            );
            let r = board.node(&edge.from.node).map(Rect::of).unwrap();
            assert!(g.start().distance(g.end()) > 10.0, "{route:?}");
            assert!(g.start_from.distance(g.end()) > 1.0 && g.end_from.distance(g.end()) > 1.0);
            let middle = cubic_at([g.start(), g.start_from, g.end_from, g.end()], 0.5);
            assert!(
                !r.contains(Shape::Rectangle, middle),
                "{route:?}: {middle:?} dentro de {r:?}"
            );
        }
    }

    #[test]
    fn hit_finds_the_topmost_node_but_not_groups() {
        let board = generar::example();
        let inside_card = P::new(100.0, 200.0);
        assert_eq!(
            hit(&board, inside_card).map(|i| board.nodes[i].id.as_str()),
            Some("nota-03")
        );
        let only_group = P::new(30.0, 420.0);
        assert_eq!(hit(&board, only_group), None);
    }
}
