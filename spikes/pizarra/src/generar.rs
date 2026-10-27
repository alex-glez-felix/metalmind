//! Pizarras de prueba: el ejemplo de `ejemplos/` y tableros deterministas
//! de N elementos para medir.

use crate::formato::{Board, Edge, End, Node, NodeKind, Route, Shape, Side, Tip};
use crate::trazo::Point;

fn node(id: &str, x: i32, y: i32, width: i32, height: i32, kind: NodeKind) -> Node {
    Node {
        id: id.to_owned(),
        x,
        y,
        width,
        height,
        color: None,
        kind,
        extra: serde_json::Map::new(),
    }
}

fn text(id: &str, x: i32, y: i32, width: i32, height: i32, shape: Shape, text: &str) -> Node {
    node(
        id,
        x,
        y,
        width,
        height,
        NodeKind::Text {
            text: text.to_owned(),
            shape,
        },
    )
}

fn card(id: &str, x: i32, y: i32, width: i32, file: &str) -> Node {
    node(
        id,
        x,
        y,
        width,
        72,
        NodeKind::File {
            file: file.to_owned(),
            subpath: None,
        },
    )
}

fn colored(mut node: Node, color: &str) -> Node {
    node.color = Some(color.to_owned());
    node
}

fn edge(id: &str, from: &str, to: &str) -> Edge {
    Edge {
        id: id.to_owned(),
        from: End {
            node: from.to_owned(),
            side: None,
            tip: Tip::None,
        },
        to: End {
            node: to.to_owned(),
            side: None,
            tip: Tip::Arrow,
        },
        route: Route::Curve,
        color: None,
        label: None,
        extra: serde_json::Map::new(),
    }
}

fn routed(mut edge: Edge, route: Route) -> Edge {
    edge.route = route;
    edge
}

fn labeled(mut edge: Edge, label: &str) -> Edge {
    edge.label = Some(label.to_owned());
    edge
}

#[expect(clippy::cast_precision_loss, reason = "pocos puntos")]
fn circle_stroke(width: f32, height: f32, points: usize) -> Vec<Point> {
    // Una vuelta y un poco más, como cuando se encierra algo a mano.
    (0..points)
        .map(|i| {
            // Arranca arriba a la izquierda, se pasa un poco y no cierra justo.
            let t = 2.4 + i as f32 / (points - 1) as f32 * std::f32::consts::TAU * 1.12;
            let r = 1.0 + 0.07 * (t * 2.0).sin() + 0.04 * (t - 2.4);
            Point {
                x: width / 2.0 * (1.0 - t.cos() * r),
                y: height / 2.0 * (1.0 - t.sin() * r),
                pressure: None,
            }
        })
        .collect()
}

/// Una pizarra de arquitectura como las que motivan el spike: tarjetas de
/// notas del vault, cajas con forma, un grupo, flechas con y sin etiqueta,
/// un trazo a mano y textos largos.
#[must_use]
pub fn example() -> Board {
    Board {
        nodes: [example_frame(), example_boxes()].concat(),
        edges: example_edges(),
    }
}

/// El grupo, el título y las tarjetas de las notas de investigación.
fn example_frame() -> Vec<Node> {
    vec![
        node(
            "capas",
            20,
            140,
            880,
            300,
            NodeKind::Group {
                label: Some("Núcleo, sin GPUI".to_owned()),
            },
        ),
        text(
            "titulo",
            20,
            20,
            900,
            60,
            Shape::None,
            "# Cómo entra la pizarra en Metalmind",
        ),
        card(
            "nota-03",
            60,
            180,
            300,
            "docs/research/03-core-architecture.md",
        ),
        card(
            "nota-07",
            60,
            520,
            300,
            "docs/research/07-features-otras-apps.md",
        ),
    ]
}

/// Las cajas con forma, el texto largo, la tarjeta de ruta larga, el trazo y el apunte.
fn example_boxes() -> Vec<Node> {
    vec![
        colored(
            text(
                "formato",
                440,
                180,
                220,
                90,
                Shape::Rectangle,
                "**metalmind-board**\nformato y geometría",
            ),
            "4",
        ),
        colored(
            text(
                "vault",
                440,
                330,
                220,
                80,
                Shape::Rectangle,
                "metalmind-vault\nbacklinks y tags",
            ),
            "5",
        ),
        colored(
            text(
                "indice",
                700,
                300,
                170,
                110,
                Shape::Ellipse,
                "índice de links",
            ),
            "6",
        ),
        colored(
            text(
                "ui",
                1040,
                180,
                240,
                90,
                Shape::Rectangle,
                "metalmind-ui\nvista y edición",
            ),
            "1",
        ),
        colored(
            text(
                "decide",
                1060,
                360,
                200,
                140,
                Shape::Diamond,
                "¿cabe en\nun frame?",
            ),
            "3",
        ),
        text(
            "nota-larga",
            440,
            520,
            560,
            110,
            Shape::Rectangle,
            "Un texto largo a propósito dentro de una caja: las flechas se pegan a las formas, se re-enrutan al mover y nunca quedan colgando, y el archivo cambia una sola línea por cada caja que se mueve.",
        ),
        card(
            "nota-ruta-larga",
            1040,
            560,
            320,
            "proyectos/metalmind/diseño/una-nota-con-un-nombre-muy-largo-para-ver-el-recorte.md",
        ),
        node(
            "rodeo",
            420,
            160,
            260,
            130,
            NodeKind::Stroke {
                stroke_width: 2.0,
                points: circle_stroke(260.0, 130.0, 90),
            },
        ),
        text(
            "apunte",
            700,
            92,
            220,
            60,
            Shape::None,
            "lo nuevo: un crate\ncon el formato",
        ),
    ]
}

fn example_edges() -> Vec<Edge> {
    vec![
        labeled(edge("e1", "nota-03", "formato"), "§6: núcleo"),
        edge("e2", "formato", "vault"),
        routed(edge("e3", "vault", "indice"), Route::Straight),
        labeled(edge("e4", "formato", "ui"), "solo la UI importa GPUI"),
        routed(edge("e5", "ui", "decide"), Route::Elbow),
        Edge {
            from: End {
                node: "decide".to_owned(),
                side: Some(Side::Bottom),
                tip: Tip::None,
            },
            ..routed(edge("e6", "decide", "nota-ruta-larga"), Route::Elbow)
        },
        labeled(edge("e7", "nota-07", "nota-larga"), "revisa «No encaja»"),
        routed(edge("e8", "nota-03", "nota-07"), Route::Straight),
    ]
}

/// Generador xorshift64*: determinista y sin dependencias.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn range(&mut self, from: i32, to: i32) -> i32 {
        let span = u64::from((to - from).unsigned_abs()).max(1);
        from + i32::try_from(self.next() % span).unwrap_or(0)
    }
}

/// Elementos por grupo del tablero: seis nodos (tres cajas con forma, una
/// tarjeta, un texto suelto y un trazo) y cuatro flechas.
const PER_GROUP: usize = 10;

/// Un tablero de `n` elementos (nodos más flechas) en grupos de ideas. Con
/// la misma semilla sale el mismo archivo.
#[must_use]
pub fn board(n: usize, seed: u64) -> Board {
    let mut rng = Rng(seed.max(1));
    let groups = n.div_ceil(PER_GROUP);
    let columns = (1..=groups).find(|c| c * c >= groups).unwrap_or(1);
    let mut board = Board::default();
    let mut count = 0;
    for g in 0..groups {
        let (ox, oy) = (
            i32::try_from(g % columns).unwrap_or(0) * 900,
            i32::try_from(g / columns).unwrap_or(0) * 640,
        );
        let id = |name: &str| format!("g{g}-{name}");
        let nodes = [
            card(
                &id("nota"),
                ox + 40,
                oy + 40,
                rng.range(220, 320),
                &format!("ideas/idea-{g:04}.md"),
            ),
            colored(
                text(
                    &id("caja"),
                    ox + 460,
                    oy + 40,
                    rng.range(180, 260),
                    90,
                    Shape::Rectangle,
                    "Una caja con\ntexto en dos líneas",
                ),
                "4",
            ),
            text(
                &id("elipse"),
                ox + 460,
                oy + 260,
                200,
                rng.range(90, 130),
                Shape::Ellipse,
                "¿y si…?",
            ),
            colored(
                text(
                    &id("rombo"),
                    ox + 120,
                    oy + 260,
                    180,
                    120,
                    Shape::Diamond,
                    "¿sí o no?",
                ),
                "3",
            ),
            text(
                &id("apunte"),
                ox + 40,
                oy + 440,
                300,
                60,
                Shape::None,
                "Un apunte suelto que\nocupa dos líneas",
            ),
            node(
                &id("trazo"),
                ox + 460,
                oy + 440,
                300,
                80,
                NodeKind::Stroke {
                    stroke_width: 2.0,
                    points: scribble(&mut rng),
                },
            ),
        ];
        let edges = [
            labeled(edge(&id("e1"), &id("nota"), &id("caja")), "sigue"),
            edge(&id("e2"), &id("caja"), &id("elipse")),
            routed(edge(&id("e3"), &id("elipse"), &id("rombo")), Route::Elbow),
            routed(edge(&id("e4"), &id("nota"), &id("rombo")), Route::Straight),
        ];
        for node in nodes {
            if count < n {
                board.nodes.push(node);
                count += 1;
            }
        }
        for edge in edges {
            if count < n {
                board.edges.push(edge);
                count += 1;
            }
        }
    }
    board
}

/// Un garabato de 80 puntos, como un subrayado a mano.
fn scribble(rng: &mut Rng) -> Vec<Point> {
    let (mut x, mut y) = (0.0_f32, 40.0_f32);
    (0..80)
        .map(|_| {
            #[expect(clippy::cast_precision_loss, reason = "pasos chicos")]
            {
                x += rng.range(10, 40) as f32 / 10.0;
                y = (y + rng.range(-50, 50) as f32 / 10.0).clamp(0.0, 80.0);
            }
            Point {
                x,
                y,
                pressure: None,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_board_has_exactly_n_elements() {
        for n in [1, 9, 10, 500, 2000] {
            let b = board(n, 1);
            assert_eq!(b.nodes.len() + b.edges.len(), n);
        }
    }

    #[test]
    fn edges_only_point_to_nodes_that_exist() {
        for n in [7, 500] {
            let text = board(n, 1).write();
            assert!(Board::parse(&text).is_ok(), "{n}");
        }
    }

    #[test]
    fn the_same_seed_gives_the_same_file() {
        let write = |seed| board(100, seed).write();
        assert_eq!(write(3), write(3));
        assert_ne!(write(3), write(4));
    }

    #[test]
    fn the_example_reads_back_the_same() {
        let text = example().write();
        assert_eq!(Board::parse(&text).unwrap().write(), text);
    }
}
