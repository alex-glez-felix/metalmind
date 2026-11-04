//! Un nodo de la pizarra y su línea en el archivo. Las claves que su tipo no
//! usa son desconocidas para él y se conservan, aunque otro tipo sí las use.

use std::fmt::Write as _;

use serde::de::{self, DeserializeOwned};
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value};

use crate::board::{FormatError, write_extra};
use crate::stroke_points::{self, StrokePoint};

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
    /// Las claves que la pizarra no conoce (de Obsidian o de un plugin).
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
        points: Vec<StrokePoint>,
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

impl Shape {
    /// `None` en un rectángulo: es la caja de JSON Canvas y no se escribe.
    fn extension_name(self) -> Option<&'static str> {
        match self {
            Self::Rectangle => None,
            Self::Ellipse => Some("ellipse"),
            Self::Diamond => Some("diamond"),
            Self::None => Some("none"),
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct RawNode {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(deserialize_with = "coordinate")]
    x: i32,
    #[serde(deserialize_with = "coordinate")]
    y: i32,
    #[serde(deserialize_with = "coordinate")]
    width: i32,
    #[serde(deserialize_with = "coordinate")]
    height: i32,
    color: Option<String>,
    #[serde(flatten)]
    rest: Map<String, Value>,
}

/// JSON Canvas pide enteros, pero una herramienta puede escribir `10.5`: no
/// vale perder la pizarra por medio píxel.
fn coordinate<'de, D: Deserializer<'de>>(deserializer: D) -> Result<i32, D::Error> {
    let value = f64::deserialize(deserializer)?.round();
    if (f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&value) {
        #[expect(clippy::cast_possible_truncation, reason = "ya se comprobó el rango")]
        Ok(value as i32)
    } else {
        Err(de::Error::custom(format!(
            "coordenada fuera de rango: {value}"
        )))
    }
}

/// `shift_remove` y no `remove`, que movería la última clave al hueco.
fn take<T: DeserializeOwned>(
    rest: &mut Map<String, Value>,
    key: &str,
) -> Result<Option<T>, FormatError> {
    match rest.shift_remove(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => Ok(Some(serde_json::from_value(value)?)),
    }
}

fn read_kind(raw: &mut RawNode) -> Result<NodeKind, FormatError> {
    let id = raw.id.clone();
    let rest = &mut raw.rest;
    let missing = |field| FormatError::Missing {
        id: id.clone(),
        field,
    };
    match raw.kind.as_str() {
        "text" => Ok(NodeKind::Text {
            text: take(rest, "text")?.ok_or_else(|| missing("text"))?,
            shape: take(rest, "shape")?.unwrap_or_default(),
        }),
        "file" => Ok(NodeKind::File {
            file: take(rest, "file")?.ok_or_else(|| missing("file"))?,
            subpath: take(rest, "subpath")?,
        }),
        "link" => Ok(NodeKind::Link {
            url: take(rest, "url")?.ok_or_else(|| missing("url"))?,
        }),
        "group" => Ok(NodeKind::Group {
            label: take(rest, "label")?,
        }),
        "stroke" => {
            let stroke_width: f32 =
                take(rest, "strokeWidth")?.ok_or_else(|| missing("strokeWidth"))?;
            if !stroke_width.is_finite() || stroke_width <= 0.0 {
                return Err(FormatError::StrokeWidth { id });
            }
            let encoded: String = take(rest, "points")?.ok_or_else(|| missing("points"))?;
            let points = stroke_points::decode(&encoded).map_err(|source| FormatError::Points {
                id: id.clone(),
                source,
            })?;
            Ok(NodeKind::Stroke {
                stroke_width,
                points,
            })
        }
        other => Err(FormatError::UnknownType {
            id: id.clone(),
            kind: other.to_owned(),
        }),
    }
}

impl TryFrom<RawNode> for Node {
    type Error = FormatError;

    fn try_from(mut raw: RawNode) -> Result<Self, FormatError> {
        let kind = read_kind(&mut raw)?;
        Ok(Self {
            id: raw.id,
            x: raw.x,
            y: raw.y,
            width: raw.width,
            height: raw.height,
            color: raw.color,
            kind,
            extra: raw.rest,
        })
    }
}

impl Node {
    pub(crate) fn line(&self) -> String {
        let mut line = format!(
            "{{\"id\":{},\"type\":\"{}\",\"x\":{},\"y\":{},\"width\":{},\"height\":{}",
            Value::from(self.id.as_str()),
            self.type_name(),
            self.x,
            self.y,
            self.width,
            self.height
        );
        self.write_kind_fields(&mut line);
        if let Some(color) = &self.color {
            let _ = write!(line, ",\"color\":{}", Value::from(color.as_str()));
        }
        // Los puntos al final: lo que se lee de un diff queda antes.
        if let NodeKind::Stroke { points, .. } = &self.kind {
            let encoded = stroke_points::encode(points);
            let _ = write!(line, ",\"points\":\"{encoded}\"");
        }
        write_extra(&mut line, &self.extra);
        line.push('}');
        line
    }

    fn write_kind_fields(&self, line: &mut String) {
        match &self.kind {
            NodeKind::Text { text, shape } => {
                if let Some(shape) = shape.extension_name() {
                    let _ = write!(line, ",\"shape\":\"{shape}\"");
                }
                let _ = write!(line, ",\"text\":{}", Value::from(text.as_str()));
            }
            NodeKind::File { file, subpath } => {
                let _ = write!(line, ",\"file\":{}", Value::from(file.as_str()));
                if let Some(subpath) = subpath {
                    let _ = write!(line, ",\"subpath\":{}", Value::from(subpath.as_str()));
                }
            }
            NodeKind::Link { url } => {
                let _ = write!(line, ",\"url\":{}", Value::from(url.as_str()));
            }
            NodeKind::Group { label } => {
                if let Some(label) = label {
                    let _ = write!(line, ",\"label\":{}", Value::from(label.as_str()));
                }
            }
            NodeKind::Stroke { stroke_width, .. } => {
                let _ = write!(line, ",\"strokeWidth\":{stroke_width}");
            }
        }
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
