//! Moverse entre los nodos con el teclado, sin selección que guardar aquí: los
//! grupos son fondo y no se eligen, como en el acierto de clic.

use crate::board::Board;
use crate::edge::Side;
use crate::geometry::Point;
use crate::node::{Node, NodeKind};

/// Lo que pesa apartarse de la dirección frente a avanzar en ella: un vecino
/// casi en línea gana a uno más cercano pero muy de lado.
const SIDEWAYS_WEIGHT: f32 = 2.0;

impl Board {
    fn choosable(&self) -> impl Iterator<Item = &Node> {
        self.nodes
            .iter()
            .filter(|node| !matches!(node.kind, NodeKind::Group { .. }))
    }

    /// El nodo más cercano a `point` entre los que acepta `wanted`.
    #[must_use]
    pub fn nearest(&self, point: Point, wanted: impl Fn(&Node) -> bool) -> Option<&Node> {
        self.choosable().filter(|node| wanted(node)).min_by(|a, b| {
            let distance = |node: &Node| node.rect().center().distance(point);
            distance(a).total_cmp(&distance(b))
        })
    }

    /// El nodo más a mano hacia `side` de `from`.
    #[must_use]
    pub fn neighbor(&self, from: &Node, side: Side) -> Option<&Node> {
        let origin = from.rect().center();
        let score = |node: &Node| {
            let center = node.rect().center();
            let (dx, dy) = (center.x - origin.x, center.y - origin.y);
            let (ahead, sideways) = match side {
                Side::Left => (-dx, dy),
                Side::Right => (dx, dy),
                Side::Top => (-dy, dx),
                Side::Bottom => (dy, dx),
            };
            (ahead > 0.0).then_some(ahead + sideways.abs() * SIDEWAYS_WEIGHT)
        };
        self.choosable()
            .filter(|node| node.id != from.id)
            .filter_map(|node| Some((node, score(node)?)))
            .min_by(|(_, a), (_, b)| a.total_cmp(b))
            .map(|(node, _)| node)
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    /// ```text
    ///     arriba
    /// izq  centro  der  lejos
    ///     abajo
    /// ```
    /// y un grupo que los tapa a todos.
    const BOARD: &str = r#"{"nodes":[
        {"id":"grupo","type":"group","x":-500,"y":-500,"width":2000,"height":2000},
        {"id":"centro","type":"text","text":"","x":0,"y":0,"width":100,"height":100},
        {"id":"der","type":"text","text":"","x":200,"y":10,"width":100,"height":100},
        {"id":"lejos","type":"text","text":"","x":500,"y":0,"width":100,"height":100},
        {"id":"izq","type":"text","text":"","x":-200,"y":0,"width":100,"height":100},
        {"id":"arriba","type":"text","text":"","x":0,"y":-200,"width":100,"height":100},
        {"id":"abajo","type":"text","text":"","x":10,"y":200,"width":100,"height":100}
    ]}"#;

    #[rstest]
    #[case("centro", Side::Right, Some("der"))]
    #[case("centro", Side::Left, Some("izq"))]
    #[case("centro", Side::Top, Some("arriba"))]
    #[case("centro", Side::Bottom, Some("abajo"))]
    #[case("der", Side::Right, Some("lejos"))]
    #[case("lejos", Side::Right, None)]
    #[case("arriba", Side::Top, None)]
    fn el_vecino_es_el_mas_a_mano_hacia_ese_lado(
        #[case] from: &str,
        #[case] side: Side,
        #[case] expected: Option<&str>,
    ) {
        let board = Board::parse(BOARD).unwrap();
        let from = board.node(from).unwrap();
        let neighbor = board.neighbor(from, side).map(|node| node.id.as_str());
        assert_eq!(neighbor, expected);
    }

    #[test]
    fn un_vecino_en_linea_gana_a_uno_mas_cerca_pero_de_lado() {
        let board = Board::parse(
            r#"{"nodes":[
            {"id":"a","type":"text","text":"","x":0,"y":0,"width":10,"height":10},
            {"id":"de_lado","type":"text","text":"","x":100,"y":300,"width":10,"height":10},
            {"id":"en_linea","type":"text","text":"","x":400,"y":0,"width":10,"height":10}
        ]}"#,
        )
        .unwrap();
        let a = board.node("a").unwrap();
        assert_eq!(board.neighbor(a, Side::Right).unwrap().id, "en_linea");
    }

    #[test]
    fn lo_mas_cercano_no_es_un_grupo_y_respeta_el_filtro() {
        let board = Board::parse(BOARD).unwrap();
        let near = Point::new(250.0, 60.0);
        assert_eq!(board.nearest(near, |_| true).unwrap().id, "der");
        let not_right = board.nearest(near, |node| node.id != "der").unwrap();
        assert_eq!(not_right.id, "centro");
        assert!(board.nearest(near, |_| false).is_none());
    }
}
