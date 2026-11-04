use rstest::rstest;

use super::*;
use crate::edge::EdgeEnd;

fn example() -> Board {
    Board::parse(include_str!("../tests/fixtures/arquitectura.canvas")).unwrap()
}

fn rect(w: f32, h: f32) -> Rect {
    Rect {
        x: 0.0,
        y: 0.0,
        w,
        h,
    }
}

fn geometry_of<'a>(board: &'a Board, id: &str) -> (&'a Edge, EdgeGeometry) {
    board
        .edge_geometries()
        .find(|(edge, _)| edge.id == id)
        .unwrap()
}

#[test]
fn moving_a_box_reroutes_its_arrows() {
    let mut board = example();
    let before = geometry_of(&board, "e2").1;
    // Con `vault` a la derecha de `formato`, la flecha sale por el otro lado.
    let vault = board.nodes.iter_mut().find(|n| n.id == "vault").unwrap();
    vault.x += 600;
    vault.y -= 150;
    let after = geometry_of(&board, "e2").1;
    assert_ne!(before.start(), after.start());
    let formato = board.node("formato").unwrap().rect();
    let expected = Point::new(formato.x + formato.w, formato.center().y);
    assert!(
        after.start().distance(expected) < 1e-3,
        "{:?}",
        after.start()
    );
}

#[test]
fn an_elbow_only_turns_at_right_angles() {
    let board = example();
    let EdgePath::Polyline(points) = geometry_of(&board, "e5").1.path else {
        panic!("un codo es una polilínea");
    };
    for pair in points.windows(2) {
        let (dx, dy) = ((pair[1].x - pair[0].x).abs(), (pair[1].y - pair[0].y).abs());
        assert!(dx < 1e-3 || dy < 1e-3, "{pair:?}");
    }
}

#[test]
fn every_edge_of_the_board_gets_its_geometry() {
    let board = example();
    assert_eq!(board.edge_geometries().count(), board.edges.len());
}

#[rstest]
fn a_loop_stays_outside_its_shape_and_touches_its_outline(
    #[values(Shape::Rectangle, Shape::Ellipse, Shape::Diamond)] shape: Shape,
    #[values((200.0, 100.0), (100.0, 10_000.0), (10_000.0, 100.0), (40.0, 40.0))] size: (f32, f32),
) {
    let r = rect(size.0, size.1);
    let geometry = self_loop(&r, shape);
    let EdgePath::Cubic(curve) = geometry.path else {
        panic!("un lazo es una curva");
    };
    for i in 1..20_u8 {
        let p = cubic_at(curve, f32::from(i) / 20.0);
        assert!(!r.contains(shape, p), "{p:?} entra en la forma");
    }
    let c = r.center();
    for end in [geometry.start(), geometry.end()] {
        let (sx, sy) = ((c.x - end.x).signum(), (c.y - end.y).signum());
        assert!(
            r.contains(shape, Point::new(end.x + sx, end.y + sy)),
            "{end:?}"
        );
        assert!(
            !r.contains(shape, Point::new(end.x - sx, end.y - sy)),
            "{end:?}"
        );
    }
}

#[rstest]
fn an_arrow_to_its_own_node_loops_outside_it(
    #[values(Route::Curve, Route::Straight, Route::Elbow)] route: Route,
) {
    let board = example();
    let node = &board.nodes[1];
    let end = EdgeEnd {
        node: node.id.clone(),
        side: None,
        tip: crate::edge::Tip::Arrow,
    };
    let edge = Edge {
        from: end.clone(),
        to: end,
        route,
        ..board.edges[0].clone()
    };
    let geometry = edge.geometry(node, node);
    assert!(geometry.start().distance(geometry.end()) > 10.0);
    let middle = cubic_at(
        [
            geometry.start(),
            geometry.start_from,
            geometry.end_from,
            geometry.end(),
        ],
        0.5,
    );
    assert!(
        !node.rect().contains(Shape::Rectangle, middle),
        "{middle:?}"
    );
}
