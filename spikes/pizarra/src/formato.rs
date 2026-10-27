//! El formato en disco: JSON Canvas 1.0 (jsoncanvas.org) con cuatro
//! extensiones (`shape`, `route`, el nodo `stroke` y sus puntos), escrito con
//! un elemento por línea y las claves en un orden fijo para que un cambio
//! toque solo la línea de lo que cambió.

use std::fmt::Write as _;

use serde::Deserialize;
use serde_json::{Map, Value};
use thiserror::Error;

use crate::trazo::{self, Point};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Board {
    /// En orden de abajo hacia arriba, como pide JSON Canvas.
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    /// `"1"`…`"6"` (los colores del tema) o `#rrggbb`, como en JSON Canvas.
    pub color: Option<String>,
    pub kind: NodeKind,
    /// Las claves que la pizarra no conoce (de Obsidian o de un plugin): se
    /// escriben igual, al final de la línea.
    pub extra: Map<String, Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NodeKind {
    /// Una caja con texto en markdown; `Shape::None` es texto suelto.
    Text {
        text: String,
        shape: Shape,
    },
    /// Una tarjeta de nota: `file` es la ruta dentro del vault.
    File {
        file: String,
        subpath: Option<String>,
    },
    Link {
        url: String,
    },
    Group {
        label: Option<String>,
    },
    /// Trazo libre; los puntos son relativos a `(x, y)` del nodo.
    Stroke {
        stroke_width: f32,
        points: Vec<Point>,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Shape {
    #[default]
    Rectangle,
    Ellipse,
    Diamond,
    None,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Edge {
    pub id: String,
    pub from: End,
    pub to: End,
    pub route: Route,
    pub color: Option<String>,
    pub label: Option<String>,
    pub extra: Map<String, Value>,
}

/// Un extremo de una flecha: siempre un nodo, nunca una coordenada.
#[derive(Clone, Debug, PartialEq)]
pub struct End {
    pub node: String,
    /// `None`: el lado lo elige la geometría, y cambia al mover los nodos.
    pub side: Option<Side>,
    pub tip: Tip,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Top,
    Right,
    Bottom,
    Left,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tip {
    None,
    Arrow,
}

/// Sin `route`, la curva de Obsidian; `straight` y `elbow` son extensiones.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Route {
    #[default]
    Curve,
    Straight,
    Elbow,
}

#[derive(Debug, Error)]
pub enum FormatError {
    #[error("JSON inválido: {0}")]
    Json(#[from] serde_json::Error),
    #[error("el nodo {id} es de un tipo desconocido: {kind}")]
    UnknownType { id: String, kind: String },
    #[error("al nodo {id} le falta `{field}`")]
    Missing { id: String, field: &'static str },
    #[error("los puntos del trazo {id} no se leen: {source}")]
    Points {
        id: String,
        source: trazo::PointsError,
    },
    #[error("la flecha {edge} apunta a un nodo que no existe: {node}")]
    DanglingEdge { edge: String, node: String },
}

#[derive(Deserialize)]
struct RawBoard {
    #[serde(default)]
    nodes: Vec<RawNode>,
    #[serde(default)]
    edges: Vec<RawEdge>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawNode {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    color: Option<String>,
    text: Option<String>,
    shape: Option<Shape>,
    file: Option<String>,
    subpath: Option<String>,
    url: Option<String>,
    label: Option<String>,
    stroke_width: Option<f32>,
    points: Option<String>,
    #[serde(flatten)]
    extra: Map<String, Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawEdge {
    id: String,
    from_node: String,
    from_side: Option<Side>,
    from_end: Option<Tip>,
    to_node: String,
    to_side: Option<Side>,
    to_end: Option<Tip>,
    route: Option<Route>,
    color: Option<String>,
    label: Option<String>,
    #[serde(flatten)]
    extra: Map<String, Value>,
}

impl Board {
    /// Lee un `.canvas`; también los de Obsidian, que no usan extensiones.
    ///
    /// # Errors
    ///
    /// JSON inválido, un nodo sin sus campos, puntos que no se leen o una
    /// flecha hacia un nodo que no está.
    pub fn parse(json: &str) -> Result<Self, FormatError> {
        let raw: RawBoard = serde_json::from_str(json)?;
        let nodes = raw
            .nodes
            .into_iter()
            .map(Node::from_raw)
            .collect::<Result<Vec<_>, _>>()?;
        let edges: Vec<Edge> = raw.edges.into_iter().map(Edge::from).collect();
        for edge in &edges {
            for end in [&edge.from, &edge.to] {
                if !nodes.iter().any(|node| node.id == end.node) {
                    return Err(FormatError::DanglingEdge {
                        edge: edge.id.clone(),
                        node: end.node.clone(),
                    });
                }
            }
        }
        Ok(Self { nodes, edges })
    }

    /// Escribe el archivo canónico: un elemento por línea, claves en orden
    /// fijo y los puntos de un trazo al final de su propia línea.
    #[must_use]
    pub fn write(&self) -> String {
        let nodes: Vec<String> = self.nodes.iter().map(Node::line).collect();
        let edges: Vec<String> = self.edges.iter().map(Edge::line).collect();
        format!(
            "{{\n\t\"nodes\":[\n{}\n\t],\n\t\"edges\":[\n{}\n\t]\n}}\n",
            nodes.join(",\n"),
            edges.join(",\n")
        )
    }

    #[must_use]
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|node| node.id == id)
    }
}

impl Node {
    fn from_raw(raw: RawNode) -> Result<Self, FormatError> {
        let id = raw.id.clone();
        let missing = |field| FormatError::Missing {
            id: id.clone(),
            field,
        };
        let kind = match raw.kind.as_str() {
            "text" => NodeKind::Text {
                text: raw.text.ok_or_else(|| missing("text"))?,
                shape: raw.shape.unwrap_or_default(),
            },
            "file" => NodeKind::File {
                file: raw.file.ok_or_else(|| missing("file"))?,
                subpath: raw.subpath,
            },
            "link" => NodeKind::Link {
                url: raw.url.ok_or_else(|| missing("url"))?,
            },
            "group" => NodeKind::Group { label: raw.label },
            "stroke" => {
                let encoded = raw.points.ok_or_else(|| missing("points"))?;
                let points = trazo::decode(&encoded).map_err(|source| FormatError::Points {
                    id: id.clone(),
                    source,
                })?;
                NodeKind::Stroke {
                    stroke_width: raw.stroke_width.ok_or_else(|| missing("strokeWidth"))?,
                    points,
                }
            }
            other => {
                return Err(FormatError::UnknownType {
                    id: raw.id,
                    kind: other.to_owned(),
                });
            }
        };
        Ok(Self {
            id: raw.id,
            x: raw.x,
            y: raw.y,
            width: raw.width,
            height: raw.height,
            color: raw.color,
            kind,
            extra: raw.extra,
        })
    }

    fn line(&self) -> String {
        let mut line = format!(
            "\t\t{{\"id\":{},\"type\":\"{}\",\"x\":{},\"y\":{},\"width\":{},\"height\":{}",
            quote(&self.id),
            self.type_name(),
            self.x,
            self.y,
            self.width,
            self.height
        );
        match &self.kind {
            NodeKind::Text { text, shape } => {
                if let Some(shape) = shape_name(*shape) {
                    let _ = write!(line, ",\"shape\":\"{shape}\"");
                }
                let _ = write!(line, ",\"text\":{}", quote(text));
            }
            NodeKind::File { file, subpath } => {
                let _ = write!(line, ",\"file\":{}", quote(file));
                if let Some(subpath) = subpath {
                    let _ = write!(line, ",\"subpath\":{}", quote(subpath));
                }
            }
            NodeKind::Link { url } => {
                let _ = write!(line, ",\"url\":{}", quote(url));
            }
            NodeKind::Group { label } => {
                if let Some(label) = label {
                    let _ = write!(line, ",\"label\":{}", quote(label));
                }
            }
            NodeKind::Stroke { stroke_width, .. } => {
                let _ = write!(line, ",\"strokeWidth\":{stroke_width}");
            }
        }
        if let Some(color) = &self.color {
            let _ = write!(line, ",\"color\":{}", quote(color));
        }
        if let NodeKind::Stroke { points, .. } = &self.kind {
            let _ = write!(line, ",\"points\":{}", quote(&trazo::encode(points)));
        }
        write_extra(&mut line, &self.extra);
        line.push('}');
        line
    }

    fn type_name(&self) -> &'static str {
        match self.kind {
            NodeKind::Text { .. } => "text",
            NodeKind::File { .. } => "file",
            NodeKind::Link { .. } => "link",
            NodeKind::Group { .. } => "group",
            NodeKind::Stroke { .. } => "stroke",
        }
    }
}

impl From<RawEdge> for Edge {
    fn from(raw: RawEdge) -> Self {
        Self {
            id: raw.id,
            from: End {
                node: raw.from_node,
                side: raw.from_side,
                tip: raw.from_end.unwrap_or(Tip::None),
            },
            to: End {
                node: raw.to_node,
                side: raw.to_side,
                tip: raw.to_end.unwrap_or(Tip::Arrow),
            },
            route: raw.route.unwrap_or_default(),
            color: raw.color,
            label: raw.label,
            extra: raw.extra,
        }
    }
}

impl Edge {
    fn line(&self) -> String {
        let mut line = format!(
            "\t\t{{\"id\":{},\"fromNode\":{}",
            quote(&self.id),
            quote(&self.from.node)
        );
        if let Some(side) = self.from.side {
            let _ = write!(line, ",\"fromSide\":\"{}\"", side_name(side));
        }
        if self.from.tip == Tip::Arrow {
            line.push_str(",\"fromEnd\":\"arrow\"");
        }
        let _ = write!(line, ",\"toNode\":{}", quote(&self.to.node));
        if let Some(side) = self.to.side {
            let _ = write!(line, ",\"toSide\":\"{}\"", side_name(side));
        }
        if self.to.tip == Tip::None {
            line.push_str(",\"toEnd\":\"none\"");
        }
        match self.route {
            Route::Curve => {}
            Route::Straight => line.push_str(",\"route\":\"straight\""),
            Route::Elbow => line.push_str(",\"route\":\"elbow\""),
        }
        if let Some(color) = &self.color {
            let _ = write!(line, ",\"color\":{}", quote(color));
        }
        if let Some(label) = &self.label {
            let _ = write!(line, ",\"label\":{}", quote(label));
        }
        write_extra(&mut line, &self.extra);
        line.push('}');
        line
    }
}

fn write_extra(line: &mut String, extra: &Map<String, Value>) {
    for (key, value) in extra {
        let _ = write!(line, ",{}:{value}", quote(key));
    }
}

fn shape_name(shape: Shape) -> Option<&'static str> {
    match shape {
        Shape::Rectangle => None,
        Shape::Ellipse => Some("ellipse"),
        Shape::Diamond => Some("diamond"),
        Shape::None => Some("none"),
    }
}

fn side_name(side: Side) -> &'static str {
    match side {
        Side::Top => "top",
        Side::Right => "right",
        Side::Bottom => "bottom",
        Side::Left => "left",
    }
}

fn quote(text: &str) -> String {
    serde_json::Value::from(text).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generar;

    /// Un `.canvas` como lo guarda Obsidian: sin extensiones, una clave que
    /// la pizarra no conoce y un grupo con fondo.
    const FROM_OBSIDIAN: &str = r#"{
	"nodes":[
		{"id":"a1","type":"text","text":"Hola","x":-340,"y":-200,"width":250,"height":60},
		{"id":"b2","type":"file","file":"ideas/pizarra.md","x":0,"y":0,"width":400,"height":400,"color":"4"},
		{"id":"c3","type":"group","x":-400,"y":-260,"width":900,"height":700,"label":"Todo","background":"fondo.png","backgroundStyle":"cover"}
	],
	"edges":[
		{"id":"e1","fromNode":"a1","fromSide":"right","toNode":"b2","toSide":"left"}
	]
}"#;

    #[test]
    fn reads_what_obsidian_writes_and_keeps_unknown_keys() {
        let board = Board::parse(FROM_OBSIDIAN).unwrap();
        assert_eq!(board.nodes.len(), 3);
        assert_eq!(board.edges[0].from.side, Some(Side::Right));
        assert_eq!(board.edges[0].to.tip, Tip::Arrow);
        let written = board.write();
        assert!(
            written.contains(r#""background":"fondo.png","backgroundStyle":"cover"}"#),
            "{written}"
        );
        assert_eq!(Board::parse(&written).unwrap(), board);
    }

    #[test]
    fn writing_twice_gives_the_same_bytes() {
        let once = generar::example().write();
        assert_eq!(Board::parse(&once).unwrap().write(), once);
    }

    #[test]
    fn moving_a_box_changes_only_its_line() {
        let mut board = generar::example();
        let before = board.write();
        let moved = board.nodes.iter_mut().find(|n| n.id == "formato").unwrap();
        moved.x += 40;
        moved.y -= 20;
        let after = board.write();
        let changed: Vec<_> = similar::TextDiff::from_lines(&before, &after)
            .iter_all_changes()
            .filter(|c| c.tag() == similar::ChangeTag::Insert)
            .map(|c| c.value().trim().to_owned())
            .collect();
        assert_eq!(changed.len(), 1, "{changed:?}");
        assert!(
            changed[0].starts_with(r#"{"id":"formato","type":"text","x":480,"y":160,"#),
            "{}",
            changed[0]
        );
    }

    #[test]
    fn an_edge_to_a_missing_node_is_an_error() {
        let text = FROM_OBSIDIAN.replace(r#""toNode":"b2""#, r#""toNode":"zz""#);
        assert!(matches!(
            Board::parse(&text),
            Err(FormatError::DanglingEdge { .. })
        ));
    }

    #[test]
    fn unknown_types_and_missing_fields_are_errors() {
        let unknown = FROM_OBSIDIAN.replace(r#""type":"text""#, r#""type":"video""#);
        assert!(matches!(
            Board::parse(&unknown),
            Err(FormatError::UnknownType { .. })
        ));
        let missing = FROM_OBSIDIAN.replace(r#""file":"ideas/pizarra.md","#, "");
        assert!(matches!(
            Board::parse(&missing),
            Err(FormatError::Missing { field: "file", .. })
        ));
    }

    #[test]
    fn extensions_are_written_only_when_they_differ_from_json_canvas() {
        let board = generar::example();
        let written = board.write();
        let line = |id: &str| {
            written
                .lines()
                .find(|l| l.contains(&format!("\"id\":\"{id}\"")))
                .unwrap_or_default()
                .to_owned()
        };
        assert!(
            !line("formato").contains("\"shape\""),
            "un rectángulo es el text de JSON Canvas"
        );
        assert!(line("indice").contains(r#""shape":"ellipse""#));
        assert!(
            !line("e2").contains("\"route\""),
            "una curva es la flecha de JSON Canvas"
        );
        assert!(line("e3").contains(r#""route":"straight""#));
        assert!(
            line("rodeo").contains(r#""type":"stroke""#) && line("rodeo").contains(r#""points":"#)
        );
    }
}
