//! Una flecha de la pizarra y su línea en el archivo. No guarda coordenadas:
//! se calculan de los nodos que une.

use std::fmt::Write as _;

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::board::write_extra;

#[derive(Clone, Debug, PartialEq)]
pub struct Edge {
    pub id: String,
    pub from: EdgeEnd,
    pub to: EdgeEnd,
    pub route: Route,
    pub color: Option<String>,
    pub label: Option<String>,
    pub extra: Map<String, Value>,
}

/// Un extremo de una flecha: siempre un nodo, nunca una coordenada.
#[derive(Clone, Debug, PartialEq)]
pub struct EdgeEnd {
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

impl Side {
    fn name(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Right => "right",
            Self::Bottom => "bottom",
            Self::Left => "left",
        }
    }
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RawEdge {
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

impl From<RawEdge> for Edge {
    /// Una flecha de un nodo a sí mismo es siempre un lazo: su `route` no
    /// cambia nada, así que no se guarda.
    fn from(raw: RawEdge) -> Self {
        let route = if raw.from_node == raw.to_node {
            Route::Curve
        } else {
            raw.route.unwrap_or_default()
        };
        Self {
            id: raw.id,
            from: EdgeEnd {
                node: raw.from_node,
                side: raw.from_side,
                tip: raw.from_end.unwrap_or(Tip::None),
            },
            to: EdgeEnd {
                node: raw.to_node,
                side: raw.to_side,
                tip: raw.to_end.unwrap_or(Tip::Arrow),
            },
            route,
            color: raw.color,
            label: raw.label,
            extra: raw.extra,
        }
    }
}

impl Edge {
    /// Las puntas solo se escriben si difieren de lo que JSON Canvas asume.
    pub(crate) fn line(&self) -> String {
        let mut line = format!(
            "{{\"id\":{},\"fromNode\":{}",
            Value::from(self.id.as_str()),
            Value::from(self.from.node.as_str())
        );
        if let Some(side) = self.from.side {
            let _ = write!(line, ",\"fromSide\":\"{}\"", side.name());
        }
        if self.from.tip == Tip::Arrow {
            line.push_str(",\"fromEnd\":\"arrow\"");
        }
        let _ = write!(line, ",\"toNode\":{}", Value::from(self.to.node.as_str()));
        if let Some(side) = self.to.side {
            let _ = write!(line, ",\"toSide\":\"{}\"", side.name());
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
            let _ = write!(line, ",\"color\":{}", Value::from(color.as_str()));
        }
        if let Some(label) = &self.label {
            let _ = write!(line, ",\"label\":{}", Value::from(label.as_str()));
        }
        write_extra(&mut line, &self.extra);
        line.push('}');
        line
    }
}
