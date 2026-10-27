//! Esbozo del mapeo entre la pizarra y `.excalidraw`, para un convertidor
//! fuera de la app (`src/bin/convertir.rs`) y para comparar los dos formatos
//! con el mismo contenido.

use serde_json::{Map, Value, json};

use crate::excalidraw::{Element, Scene, number};
use crate::formato::{Board, Edge, End, Node, NodeKind, Route, Shape, Tip};
use crate::geometria::{self, EdgePath, P};
use crate::trazo::Point;

/// Los colores con número de JSON Canvas, en la paleta de Excalidraw.
const PALETTE: [&str; 6] = [
    "#e03131", "#f08c00", "#f2c94c", "#2f9e44", "#1971c2", "#9c36b5",
];
const INK: &str = "#1e1e1e";

fn stroke_color(color: Option<&str>) -> String {
    match color {
        Some(hex) if hex.starts_with('#') => hex.to_owned(),
        Some(n) => n
            .parse::<usize>()
            .ok()
            .and_then(|i| PALETTE.get(i.wrapping_sub(1)))
            .map_or_else(|| INK.to_owned(), |c| (*c).to_owned()),
        None => INK.to_owned(),
    }
}

/// Un elemento de Excalidraw 0.18 con todos sus campos, en el orden en que
/// los escribe.
struct Draft(Map<String, Value>);

impl Draft {
    fn new(id: &str, kind: &str, x: f32, y: f32, width: f32, height: f32, color: &str) -> Self {
        let seed = fnv(id) >> 33;
        let Value::Object(map) = json!({
            "id": id, "type": kind,
            "x": number(f64::from(x)), "y": number(f64::from(y)),
            "width": number(f64::from(width)), "height": number(f64::from(height)),
            "angle": 0, "strokeColor": color, "backgroundColor": "transparent",
            "fillStyle": "solid", "strokeWidth": 2, "strokeStyle": "solid",
            "roughness": 1, "opacity": 100, "groupIds": [], "frameId": null,
            "index": null, "roundness": null, "seed": seed, "version": 1,
            "versionNonce": seed / 3, "isDeleted": false, "boundElements": null,
            "updated": 1_761_436_800_000_u64, "link": null, "locked": false,
        }) else {
            return Self(Map::new());
        };
        Self(map)
    }

    fn set(mut self, key: &str, value: Value) -> Self {
        self.0.insert(key.into(), value);
        self
    }

    /// El texto contenido en `container`, centrado como lo deja Excalidraw.
    fn text_in(
        id: &str,
        container: Option<(&str, [f32; 4])>,
        x: f32,
        y: f32,
        text: &str,
        color: &str,
    ) -> Self {
        let lines = text.lines().count().max(1);
        let longest = text.lines().map(|l| l.chars().count()).max().unwrap_or(0);
        #[expect(
            clippy::cast_precision_loss,
            reason = "un texto no tiene millones de líneas"
        )]
        let (width, height) = (longest as f32 * 20.0 * 0.55, lines as f32 * 25.0);
        let (x, y, align, valign, container_id) = match container {
            Some((cid, [cx, cy, cw, ch])) => (
                cx + (cw - width) / 2.0,
                cy + (ch - height) / 2.0,
                "center",
                "middle",
                Value::from(cid),
            ),
            None => (x, y, "left", "top", Value::Null),
        };
        Self::new(id, "text", x, y, width, height, color)
            .set("text", json!(text))
            .set("fontSize", json!(20))
            .set("fontFamily", json!(5))
            .set("textAlign", json!(align))
            .set("verticalAlign", json!(valign))
            .set("containerId", container_id)
            .set("originalText", json!(text))
            .set("autoResize", json!(true))
            .set("lineHeight", json!(1.25))
    }
}

fn fnv(id: &str) -> u64 {
    id.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    })
}

/// La pizarra como la dibujaría Excalidraw: cada caja con su texto
/// contenido, las flechas pegadas con sus puntos y los trazos con los suyos.
#[must_use]
pub fn to_excalidraw(board: &Board) -> Scene {
    let ids = board
        .nodes
        .iter()
        .map(|n| &n.id)
        .chain(board.edges.iter().map(|e| &e.id));
    let mut out = Exporter::new(ids.cloned());
    let mut drafts: Vec<Draft> = Vec::new();
    for node in &board.nodes {
        drafts.extend(node_drafts(node, &mut out));
    }
    for edge in &board.edges {
        if let Some(geometry) = geometria::route(board, edge) {
            drafts.extend(edge_drafts(
                edge,
                &geometry.path,
                geometry.label_at,
                &mut out,
            ));
        }
    }
    let mut scene = Scene::new();
    for (index, draft) in drafts.into_iter().enumerate() {
        let id = draft
            .0
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let mut draft = draft.set("index", json!(fractional_index(index)));
        if let Some(list) = out.bound.get(&id) {
            draft = draft.set("boundElements", Value::Array(list.clone()));
        }
        scene.push(draft.0);
    }
    scene
}

/// Lo que se va sabiendo al exportar: los ids ya usados (un texto generado
/// no puede repetir uno) y qué elementos quedan pegados a cada forma.
struct Exporter {
    taken: std::collections::HashSet<String>,
    bound: std::collections::HashMap<String, Vec<Value>>,
}

impl Exporter {
    fn new(ids: impl IntoIterator<Item = String>) -> Self {
        Self {
            taken: ids.into_iter().collect(),
            bound: std::collections::HashMap::new(),
        }
    }

    fn bind(&mut self, owner: &str, kind: &str, id: &str) {
        self.bound
            .entry(owner.to_owned())
            .or_default()
            .push(json!({"type": kind, "id": id}));
    }

    /// `a-texto`, o `a-texto-2` si ya existe.
    fn label_id(&mut self, owner: &str) -> String {
        let base = format!("{owner}-texto");
        let id = std::iter::once(base.clone())
            // Con `taken.len() + 2` candidatos, alguno está libre.
            .chain((2..=self.taken.len() + 2).map(|n| format!("{base}-{n}")))
            .find(|id| !self.taken.contains(id))
            .unwrap_or(base);
        self.taken.insert(id.clone());
        id
    }
}

/// Agrega un nodo al final de una escena, como lo haría Excalidraw: con el
/// índice siguiente y sin tocar los demás elementos.
pub fn push_node(scene: &mut Scene, node: &Node) {
    let elements = scene.elements();
    let mut out = Exporter::new(elements.iter().map(|e| e.id.clone()));
    let next = elements.len();
    for (offset, draft) in node_drafts(node, &mut out).into_iter().enumerate() {
        scene.push(draft.set("index", json!(fractional_index(next + offset))).0);
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "las coordenadas de una pizarra caben en f32"
)]
fn node_drafts(node: &Node, out: &mut Exporter) -> Vec<Draft> {
    let (x, y, w, h) = (
        node.x as f32,
        node.y as f32,
        node.width as f32,
        node.height as f32,
    );
    let color = stroke_color(node.color.as_deref());
    let boxed = |kind: &str, text: &str, link: Option<String>, out: &mut Exporter| {
        let text_id = out.label_id(&node.id);
        out.bind(&node.id, "text", &text_id);
        let shape = Draft::new(&node.id, kind, x, y, w, h, &color)
            .set(
                "roundness",
                if kind == "rectangle" {
                    json!({"type": 3})
                } else {
                    Value::Null
                },
            )
            .set("link", link.map_or(Value::Null, Value::from));
        let label = Draft::text_in(&text_id, Some((&node.id, [x, y, w, h])), x, y, text, &color);
        vec![shape, label]
    };
    match &node.kind {
        NodeKind::Text {
            text,
            shape: Shape::None,
        } => vec![Draft::text_in(&node.id, None, x, y, text, &color)],
        NodeKind::Text { text, shape } => {
            let kind = match shape {
                Shape::Ellipse => "ellipse",
                Shape::Diamond => "diamond",
                Shape::Rectangle | Shape::None => "rectangle",
            };
            boxed(kind, text, None, out)
        }
        NodeKind::File { file, subpath } => {
            // Con la extensión: `[[v1.2]]` sería ambiguo al volver.
            let name = file.strip_suffix(".md").unwrap_or(file);
            let target = format!("[[{file}{}]]", subpath.as_deref().unwrap_or(""));
            boxed(
                "rectangle",
                name.rsplit('/').next().unwrap_or(name),
                Some(target),
                out,
            )
        }
        NodeKind::Link { url } => boxed("rectangle", url, Some(url.clone()), out),
        NodeKind::Group { label } => {
            vec![
                Draft::new(&node.id, "frame", x, y, w, h, INK)
                    .set("name", label.clone().map_or(Value::Null, Value::from)),
            ]
        }
        NodeKind::Stroke {
            stroke_width,
            points,
        } => {
            let points: Vec<Value> = points
                .iter()
                .map(|p| json!([number(f64::from(p.x)), number(f64::from(p.y))]))
                .collect();
            vec![
                Draft::new(&node.id, "freedraw", x, y, w, h, &color)
                    .set("strokeWidth", number(f64::from(*stroke_width)))
                    .set("points", Value::Array(points))
                    .set("pressures", json!([]))
                    .set("simulatePressure", json!(true))
                    .set("lastCommittedPoint", Value::Null),
            ]
        }
    }
}

fn edge_drafts(edge: &Edge, path: &EdgePath, label_at: P, out: &mut Exporter) -> Vec<Draft> {
    let points: Vec<P> = match path {
        EdgePath::Polyline(points) => points.clone(),
        // Excalidraw guarda una flecha curva como puntos por los que pasa.
        EdgePath::Cubic(c) => [0.0, 0.25, 0.5, 0.75, 1.0]
            .map(|t| geometria::cubic_at(*c, t))
            .to_vec(),
    };
    let Some(origin) = points.first().copied() else {
        return Vec::new();
    };
    let relative: Vec<Value> = points
        .iter()
        .map(|p| {
            json!([
                number(f64::from(p.x - origin.x)),
                number(f64::from(p.y - origin.y))
            ])
        })
        .collect();
    let (min, max) = points.iter().fold((origin, origin), |(lo, hi), p| {
        (
            P::new(lo.x.min(p.x), lo.y.min(p.y)),
            P::new(hi.x.max(p.x), hi.y.max(p.y)),
        )
    });
    out.bind(&edge.from.node, "arrow", &edge.id);
    out.bind(&edge.to.node, "arrow", &edge.id);
    let color = stroke_color(edge.color.as_deref());
    let head = |tip: Tip| {
        if tip == Tip::Arrow {
            json!("arrow")
        } else {
            Value::Null
        }
    };
    let glue = |end: &End| json!({"elementId": end.node, "focus": 0, "gap": 4});
    let mut drafts = vec![
        Draft::new(
            &edge.id,
            "arrow",
            origin.x,
            origin.y,
            max.x - min.x,
            max.y - min.y,
            &color,
        )
        .set(
            "roundness",
            if matches!(path, EdgePath::Cubic(_)) {
                json!({"type": 2})
            } else {
                Value::Null
            },
        )
        .set("points", Value::Array(relative))
        .set("lastCommittedPoint", Value::Null)
        .set("startBinding", glue(&edge.from))
        .set("endBinding", glue(&edge.to))
        .set("startArrowhead", head(edge.from.tip))
        .set("endArrowhead", head(edge.to.tip))
        .set("elbowed", json!(edge.route == Route::Elbow)),
    ];
    if let Some(label) = &edge.label {
        let label_id = out.label_id(&edge.id);
        out.bind(&edge.id, "text", &label_id);
        let area = [label_at.x - 80.0, label_at.y - 12.5, 160.0, 25.0];
        drafts.push(Draft::text_in(
            &label_id,
            Some((&edge.id, area)),
            0.0,
            0.0,
            label,
            &color,
        ));
    }
    drafts
}

/// Los `index` de Excalidraw son índices fraccionarios en base 62: la
/// letra dice cuántas cifras siguen (`a0`…`az`, `b00`…`bzz`, `c000`…), así
/// que los enteros seguidos se ordenan igual como cadenas.
fn fractional_index(index: usize) -> String {
    const DIGITS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    let (mut rest, mut width, mut span) = (index, 1_u32, 62_usize);
    while rest >= span {
        rest -= span;
        width += 1;
        span = span.saturating_mul(62);
    }
    let head = char::from(b'a' + u8::try_from(width - 1).unwrap_or(25));
    let digits: String = (0..width)
        .rev()
        .map(|place| char::from(DIGITS[rest / 62_usize.pow(place) % 62]))
        .collect();
    format!("{head}{digits}")
}

/// Lo que el importador no pasa entero a la pizarra (un elemento que se
/// salta, o uno que pasa sin algo), para decírselo a quien convierte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loss {
    pub id: String,
    pub reason: &'static str,
}

/// Un `.excalidraw` como pizarra. Cajas, elipses y rombos con su texto, las
/// tarjetas (`link: "[[nota]]"`), los textos sueltos, el lápiz y las
/// flechas pegadas por los dos lados pasan; lo demás se lista en `Loss`.
#[must_use]
pub fn from_excalidraw(scene: &Scene) -> (Board, Vec<Loss>) {
    let read = scene.read();
    let elements = read.elements;
    let contained: std::collections::HashMap<&str, &Element> = elements
        .iter()
        .filter_map(|e| e.container_id.as_deref().map(|c| (c, e)))
        .collect();
    let mut board = Board::default();
    let mut skipped: Vec<Loss> = read
        .unreadable
        .into_iter()
        .map(|id| Loss {
            id,
            reason: "elemento que no se lee",
        })
        .collect();
    let mut seen = std::collections::HashSet::new();
    for element in &elements {
        if element.container_id.is_some() {
            continue;
        }
        if !seen.insert(element.id.as_str()) {
            skipped.push(Loss {
                id: element.id.clone(),
                reason: "id repetido: se quedó el primero",
            });
            continue;
        }
        if element.angle != 0.0 {
            skipped.push(Loss {
                id: element.id.clone(),
                reason: "girado: pasa sin el giro (la pizarra no gira formas)",
            });
        }
        let label = contained
            .get(element.id.as_str())
            .and_then(|t| t.text.clone());
        match import(element, label) {
            Imported::Node(node) => board.nodes.push(node),
            Imported::Edge(edge) => board.edges.push(edge),
            Imported::Skip(reason) => skipped.push(Loss {
                id: element.id.clone(),
                reason,
            }),
        }
    }
    // Una flecha cuyo extremo no pasó (p. ej. pegada a una imagen) tampoco.
    let ids: std::collections::HashSet<String> = board.nodes.iter().map(|n| n.id.clone()).collect();
    board.edges.retain(|edge| {
        let keep = ids.contains(&edge.from.node) && ids.contains(&edge.to.node);
        if !keep {
            skipped.push(Loss {
                id: edge.id.clone(),
                reason: "flecha pegada a algo que no se importó",
            });
        }
        keep
    });
    (board, skipped)
}

/// `[[nota#Sección|alias]]` es la ruta `nota.md` y el subpath `#Sección`.
/// Un nombre con extensión de letras (`datos.csv`, `mapa.canvas`) va tal
/// cual; sin extensión, o si lo que sigue al punto lleva cifras (`v1.2`), es
/// una nota. Dentro de Metalmind lo decidiría el índice del vault.
fn note_file(link: &str) -> Option<NodeKind> {
    let target = link.strip_prefix("[[")?.strip_suffix("]]")?;
    let target = target.split('|').next().unwrap_or(target);
    let (name, subpath) = match target.find('#') {
        Some(at) => (&target[..at], Some(target[at..].to_owned())),
        None => (target, None),
    };
    let last = name.rsplit('/').next().unwrap_or(name);
    let has_extension = last.rsplit_once('.').is_some_and(|(stem, ext)| {
        !stem.is_empty() && !ext.is_empty() && ext.chars().all(|c| c.is_ascii_alphabetic())
    });
    let file = if has_extension {
        name.to_owned()
    } else {
        format!("{name}.md")
    };
    Some(NodeKind::File { file, subpath })
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "coordenadas de una pizarra"
)]
fn point(x: f64, y: f64) -> Point {
    Point {
        x: x as f32,
        y: y as f32,
        pressure: None,
    }
}

/// Solo pasa una flecha pegada por los dos lados: en la pizarra no hay
/// flechas sueltas.
fn import_arrow(e: &Element, label: Option<String>, color: Option<String>) -> Imported {
    let (Some(from), Some(to)) = (&e.start_binding, &e.end_binding) else {
        return Imported::Skip("flecha suelta: en la pizarra una flecha siempre une dos nodos");
    };
    let end = |binding: &crate::excalidraw::Binding, head: &Option<String>| End {
        node: binding.element_id.clone(),
        side: None,
        tip: tip(head.as_deref()),
    };
    Imported::Edge(Edge {
        id: e.id.clone(),
        from: end(from, &e.start_arrowhead),
        to: end(to, &e.end_arrowhead),
        route: if e.elbowed {
            Route::Elbow
        } else if e.points.len() > 2 {
            Route::Curve
        } else {
            Route::Straight
        },
        color,
        label,
        extra: Map::new(),
    })
}

/// JSON Canvas solo tiene punta de flecha o nada: las puntas que señalan
/// (`arrow`, `triangle`) son flecha; las demás (punto, barra, rombo) se pierden.
fn tip(head: Option<&str>) -> Tip {
    match head {
        Some("arrow" | "triangle" | "triangle_outline") => Tip::Arrow,
        _ => Tip::None,
    }
}

enum Imported {
    Node(Node),
    Edge(Edge),
    Skip(&'static str),
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "coordenadas de una pizarra"
)]
fn import(e: &Element, label: Option<String>) -> Imported {
    let color = e
        .stroke_color
        .clone()
        .filter(|c| c != INK && c != "transparent");
    let node = |kind| {
        Imported::Node(Node {
            id: e.id.clone(),
            x: e.x.round() as i32,
            y: e.y.round() as i32,
            width: e.width.round().max(1.0) as i32,
            height: e.height.round().max(1.0) as i32,
            color: color.clone(),
            kind,
            extra: Map::new(),
        })
    };
    let text = label.clone().unwrap_or_default();
    // Un link de Obsidian es una tarjeta de nota; cualquier otro, una de link.
    let card = e.link.as_deref().map(|link| {
        note_file(link).unwrap_or_else(|| NodeKind::Link {
            url: link.to_owned(),
        })
    });
    if let ("rectangle", Some(card)) = (e.kind.as_str(), card) {
        return node(card);
    }
    match e.kind.as_str() {
        "rectangle" => node(NodeKind::Text {
            text,
            shape: Shape::Rectangle,
        }),
        "ellipse" => node(NodeKind::Text {
            text,
            shape: Shape::Ellipse,
        }),
        "diamond" => node(NodeKind::Text {
            text,
            shape: Shape::Diamond,
        }),
        "text" => node(NodeKind::Text {
            text: e.text.clone().unwrap_or_default(),
            shape: Shape::None,
        }),
        "frame" => node(NodeKind::Group {
            label: e.name.clone(),
        }),
        "freedraw" => {
            let points: Vec<Point> = e.points.iter().map(|&[x, y]| point(x, y)).collect();
            let (x, y) = (e.x as f32, e.y as f32);
            let mut stroke =
                Node::stroke(&e.id, x, y, e.stroke_width.unwrap_or(2.0) as f32, &points);
            stroke.color = color;
            Imported::Node(stroke)
        }
        "arrow" => import_arrow(e, label, color),
        "line" => Imported::Skip("línea: la pizarra no tiene líneas sueltas"),
        "image" => Imported::Skip("imagen: fuera del alcance del esbozo"),
        _ => Imported::Skip("tipo que la pizarra no conoce"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generar;

    #[test]
    fn the_example_survives_a_round_trip_through_excalidraw() {
        let board = generar::example();
        let scene = Scene::parse(&to_excalidraw(&board).write()).unwrap();
        let (back, skipped) = from_excalidraw(&scene);
        assert_eq!(skipped, []);
        assert_eq!(back.nodes.len(), board.nodes.len());
        assert_eq!(back.edges.len(), board.edges.len());
        let card = back.node("nota-03").unwrap();
        assert_eq!(
            card.kind,
            NodeKind::File {
                file: "docs/research/03-core-architecture.md".to_owned(),
                subpath: None
            }
        );
        assert_eq!(
            back.node("capas").map(|n| &n.kind),
            board.node("capas").map(|n| &n.kind)
        );
        let parsed = Board::parse(&back.write());
        assert!(parsed.is_ok(), "{parsed:?}");
    }

    fn round_trip(board: &Board) -> Board {
        let scene = Scene::parse(&to_excalidraw(board).write()).unwrap();
        let (back, skipped) = from_excalidraw(&scene);
        assert_eq!(skipped, []);
        back
    }

    #[test]
    fn cards_keep_their_kind_and_target_through_excalidraw() {
        let mut board = generar::example();
        if let Some(card) = board.nodes.iter_mut().find(|n| n.id == "nota-03") {
            card.kind = NodeKind::File {
                file: "docs/research/03-core-architecture.md".to_owned(),
                subpath: Some("#Riesgos".to_owned()),
            };
        }
        if let Some(card) = board.nodes.iter_mut().find(|n| n.id == "nota-07") {
            card.kind = NodeKind::Link {
                url: "https://jsoncanvas.org/".to_owned(),
            };
        }
        let back = round_trip(&board);
        assert_eq!(
            back.node("nota-03").map(|n| &n.kind),
            board.node("nota-03").map(|n| &n.kind)
        );
        assert_eq!(
            back.node("nota-07").map(|n| &n.kind),
            board.node("nota-07").map(|n| &n.kind)
        );
    }

    #[test]
    fn obsidian_links_become_the_right_file() {
        let file = |link: &str| match note_file(link) {
            Some(NodeKind::File { file, subpath }) => (file, subpath),
            other => panic!("{link}: {other:?}"),
        };
        assert_eq!(file("[[notas/v1.2]]"), ("notas/v1.2.md".to_owned(), None));
        assert_eq!(file("[[idea|la idea]]"), ("idea.md".to_owned(), None));
        assert_eq!(
            file("[[idea#Riesgos|riesgos]]"),
            ("idea.md".to_owned(), Some("#Riesgos".to_owned()))
        );
        assert_eq!(
            file("[[planos/casa.PDF]]"),
            ("planos/casa.PDF".to_owned(), None)
        );
        assert_eq!(file("[[mapa.canvas]]"), ("mapa.canvas".to_owned(), None));
        assert_eq!(
            file("[[datos/ventas.csv]]"),
            ("datos/ventas.csv".to_owned(), None)
        );
        assert_eq!(file("[[spec.docx]]"), ("spec.docx".to_owned(), None));
        assert_eq!(file("[[report.json]]"), ("report.json".to_owned(), None));
        assert_eq!(
            file("[[diario/2026.10.26]]"),
            ("diario/2026.10.26.md".to_owned(), None)
        );
        assert_eq!(note_file("https://jsoncanvas.org/"), None);
    }

    #[test]
    fn a_dotted_note_and_an_attachment_survive_excalidraw() {
        let mut board = generar::example();
        for (id, file) in [
            ("nota-03", "notas/v1.2.md"),
            ("nota-07", "datos/ventas.csv"),
        ] {
            if let Some(card) = board.nodes.iter_mut().find(|n| n.id == id) {
                card.kind = NodeKind::File {
                    file: file.to_owned(),
                    subpath: None,
                };
            }
        }
        let back = round_trip(&board);
        for id in ["nota-03", "nota-07"] {
            assert_eq!(
                back.node(id).map(|n| &n.kind),
                board.node(id).map(|n| &n.kind),
                "{id}"
            );
        }
    }

    #[test]
    fn arrows_keep_their_route_through_excalidraw() {
        let board = generar::example();
        let back = round_trip(&board);
        for id in ["e3", "e5", "e6"] {
            let route = |b: &Board| b.edges.iter().find(|e| e.id == id).map(|e| e.route);
            assert_eq!(route(&back), route(&board), "{id}");
        }
    }

    #[test]
    fn only_pointing_heads_become_arrow_tips() {
        assert_eq!(tip(Some("arrow")), Tip::Arrow);
        assert_eq!(tip(Some("triangle")), Tip::Arrow);
        assert_eq!(tip(Some("dot")), Tip::None);
        assert_eq!(tip(Some("bar")), Tip::None);
        assert_eq!(tip(None), Tip::None);
    }

    #[test]
    fn generated_label_ids_never_repeat_a_real_one() {
        let mut board = generar::example();
        // Un nodo cuyo id es el que tendría el texto de `formato`.
        let mut clash = board.node("vault").cloned().unwrap();
        clash.id = "formato-texto".to_owned();
        clash.y += 400;
        board.nodes.push(clash);
        let scene = to_excalidraw(&board);
        let ids: Vec<String> = scene.elements().into_iter().map(|e| e.id).collect();
        let unique: std::collections::HashSet<&String> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len());
        assert!(ids.contains(&"formato-texto-2".to_owned()), "{ids:?}");
    }

    #[test]
    fn repeated_and_rotated_elements_are_reported() {
        let mut scene = to_excalidraw(&generar::example());
        scene.push(
            Draft::new("titulo", "text", 20.0, 0.0, 10.0, 10.0, INK)
                .set("text", json!("otra"))
                .0,
        );
        scene.push(
            Draft::new("girada", "rectangle", 0.0, 900.0, 80.0, 40.0, INK)
                .set("angle", json!(0.3))
                .0,
        );
        let (board, losses) = from_excalidraw(&scene);
        let reasons: Vec<(&str, &str)> = losses.iter().map(|l| (l.id.as_str(), l.reason)).collect();
        assert!(
            reasons
                .iter()
                .any(|(id, r)| *id == "titulo" && r.starts_with("id repetido")),
            "{reasons:?}"
        );
        assert!(
            reasons
                .iter()
                .any(|(id, r)| *id == "girada" && r.starts_with("girado")),
            "{reasons:?}"
        );
        assert!(
            board.node("girada").is_some(),
            "lo girado pasa, sin el giro"
        );
        assert!(Board::parse(&board.write()).is_ok());
    }

    #[test]
    fn an_imported_stroke_stays_where_it_was() {
        let mut scene = Scene::new();
        // Puntos relativos a (100, 200) que no empiezan en 0: Excalidraw los guarda así.
        scene.push(
            Draft::new("lapiz", "freedraw", 100.0, 200.0, 40.0, 30.0, INK)
                .set("points", json!([[-10, 5], [30, -25], [12, 5]]))
                .0,
        );
        let (board, losses) = from_excalidraw(&scene);
        assert_eq!(losses, []);
        let node = board.node("lapiz").unwrap();
        let NodeKind::Stroke { points, .. } = &node.kind else {
            panic!("es un trazo");
        };
        #[expect(clippy::cast_precision_loss, reason = "coordenadas chicas")]
        let absolute: Vec<(f32, f32)> = points
            .iter()
            .map(|p| (node.x as f32 + p.x, node.y as f32 + p.y))
            .collect();
        assert_eq!(absolute, [(90.0, 205.0), (130.0, 175.0), (112.0, 205.0)]);
    }

    #[test]
    fn a_loose_arrow_is_reported_not_lost_silently() {
        let mut scene = Scene::new();
        scene.push(
            Draft::new("suelta", "arrow", 0.0, 0.0, 10.0, 0.0, INK)
                .set("points", json!([[0, 0], [10, 0]]))
                .0,
        );
        let (board, skipped) = from_excalidraw(&scene);
        assert_eq!(board.edges, []);
        assert_eq!(skipped.len(), 1);
    }

    #[test]
    fn fractional_indices_grow_as_strings() {
        let indices: Vec<_> = (0..4000).map(fractional_index).collect();
        assert!(indices.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!((indices[61].as_str(), indices[62].as_str()), ("az", "b00"));
        let at = |i: usize| fractional_index(i);
        assert_eq!((at(3905).as_str(), at(3906).as_str()), ("bzz", "c000"));
        let wide: Vec<_> = (3800..250_000).step_by(7).map(fractional_index).collect();
        assert!(wide.windows(2).all(|pair| pair[0] < pair[1]));
    }
}
