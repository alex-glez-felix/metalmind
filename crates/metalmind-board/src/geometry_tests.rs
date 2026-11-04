use rstest::rstest;

use super::*;
use crate::node::NodeKind;

fn example() -> Board {
    Board::parse(include_str!("../tests/fixtures/arquitectura.canvas")).unwrap()
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect { x, y, w, h }
}

fn close(a: Point, b: Point) -> bool {
    a.distance(b) < 1e-3
}

#[rstest]
#[case(rect(300.0, 10.0, 100.0, 50.0), Side::Right)]
#[case(rect(-300.0, 0.0, 100.0, 50.0), Side::Left)]
#[case(rect(0.0, 200.0, 100.0, 50.0), Side::Bottom)]
#[case(rect(20.0, -200.0, 100.0, 50.0), Side::Top)]
fn facing_side_looks_at_the_other_box(#[case] other: Rect, #[case] side: Side) {
    assert_eq!(rect(0.0, 0.0, 100.0, 50.0).facing_side(&other), side);
}

#[rstest]
#[case(Shape::Rectangle, Point::new(500.0, 50.0), Point::new(200.0, 50.0))]
#[case(Shape::Ellipse, Point::new(100.0, 400.0), Point::new(100.0, 100.0))]
#[case(Shape::Diamond, Point::new(-300.0, 50.0), Point::new(0.0, 50.0))]
fn boundary_toward_lands_on_each_outline(
    #[case] shape: Shape,
    #[case] target: Point,
    #[case] expected: Point,
) {
    let r = rect(0.0, 0.0, 200.0, 100.0);
    assert!(close(r.boundary_toward(shape, target), expected));
}

#[test]
fn on_a_diagonal_the_diamond_is_inside_the_ellipse_inside_the_rectangle() {
    let r = rect(0.0, 0.0, 200.0, 100.0);
    let target = Point::new(400.0, 250.0);
    let [rectangle, ellipse, diamond] = [Shape::Rectangle, Shape::Ellipse, Shape::Diamond]
        .map(|shape| r.boundary_toward(shape, target).distance(r.center()));
    assert!(rectangle > ellipse && ellipse > diamond);
    let on_ellipse = r.boundary_toward(Shape::Ellipse, target);
    let (dx, dy) = ((on_ellipse.x - 100.0) / 100.0, (on_ellipse.y - 50.0) / 50.0);
    assert!((dx * dx + dy * dy - 1.0).abs() < 1e-3);
}

#[test]
fn arrowhead_points_back_along_the_line() {
    let [tip, a, b] = arrowhead(Point::new(100.0, 0.0), Point::new(0.0, 0.0), 12.0);
    assert!(close(tip, Point::new(100.0, 0.0)));
    assert!((a.x - 88.0).abs() < 1e-3 && (b.x - 88.0).abs() < 1e-3);
    assert!((a.y + b.y).abs() < 1e-3);
}

#[test]
fn clicking_the_empty_corner_of_a_shape_misses_it() {
    let board = example();
    let indice = board.node("indice").unwrap().rect();
    let hit_id = |p| board.hit(p).map(|n| n.id.as_str());
    assert_eq!(hit_id(indice.center()), Some("indice"));
    assert_ne!(
        hit_id(Point::new(indice.x + 3.0, indice.y + 3.0)),
        Some("indice")
    );
    let decide = board.node("decide").unwrap().rect();
    assert_eq!(hit_id(Point::new(decide.x + 4.0, decide.y + 4.0)), None);
}

#[test]
fn hit_finds_the_topmost_node_but_not_groups() {
    let board = example();
    assert_eq!(
        board.hit(Point::new(100.0, 200.0)).map(|n| n.id.as_str()),
        Some("nota-03")
    );
    assert!(matches!(
        board.node("capas").unwrap().kind,
        NodeKind::Group { .. }
    ));
    assert!(board.hit(Point::new(30.0, 420.0)).is_none());
}

#[test]
fn a_zero_sized_node_has_a_box_that_routes_to_finite_points() {
    let mut board = example();
    let vault = board.nodes.iter_mut().find(|n| n.id == "vault").unwrap();
    (vault.width, vault.height) = (0, 0);
    assert_eq!((vault.rect().w, vault.rect().h), (1.0, 1.0));
    for (_, geometry) in board.edge_geometries() {
        let start = geometry.start();
        assert!(start.x.is_finite() && start.y.is_finite(), "{start:?}");
    }
}

#[test]
fn union_contains_both_rectangles() {
    assert_eq!(
        rect(0.0, 0.0, 10.0, 10.0).union(&rect(-5.0, 5.0, 2.0, 20.0)),
        rect(-5.0, 0.0, 15.0, 25.0)
    );
}
