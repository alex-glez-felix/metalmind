//! Un `.canvas` entero: leerlo, comprobar que sus elementos se pueden unir y
//! escribirlo en su forma canónica.

use std::collections::HashSet;
use std::fmt::Write as _;

use serde::Deserialize;
use serde_json::{Map, Value};
use thiserror::Error;

use crate::edge::{Edge, RawEdge};
use crate::geometry::Rect;
use crate::node::{Node, NodeKind, RawNode};
use crate::stroke_points::PointsError;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Board {
    /// De abajo hacia arriba, como pide JSON Canvas.
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    /// Lo que un plugin haya puesto junto a `nodes` y `edges`.
    pub extra: Map<String, Value>,
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
    Points { id: String, source: PointsError },
    #[error("el trazo {id} tiene un ancho que no se puede pintar")]
    StrokeWidth { id: String },
    #[error("la flecha {edge} apunta a un nodo que no existe: {node}")]
    DanglingEdge { edge: String, node: String },
    #[error("hay dos elementos con el id {0}")]
    DuplicateId(String),
}

#[derive(Deserialize)]
struct RawBoard {
    #[serde(default)]
    nodes: Vec<RawNode>,
    #[serde(default)]
    edges: Vec<RawEdge>,
    #[serde(flatten)]
    extra: Map<String, Value>,
}

impl Board {
    /// Lee un `.canvas`, también los de Obsidian, que no usan extensiones y
    /// recién creados pueden estar vacíos.
    ///
    /// # Errors
    ///
    /// JSON inválido, un nodo sin sus campos o de un tipo desconocido, un
    /// trazo que no se lee, dos elementos con el mismo id o una flecha hacia
    /// un nodo que no está.
    pub fn parse(json: &str) -> Result<Self, FormatError> {
        if json.trim().is_empty() {
            return Ok(Self::default());
        }
        let raw: RawBoard = serde_json::from_str(json)?;
        let board = Self {
            nodes: raw
                .nodes
                .into_iter()
                .map(Node::try_from)
                .collect::<Result<_, _>>()?,
            edges: raw.edges.into_iter().map(Edge::from).collect(),
            extra: raw.extra,
        };
        board.check_ids()?;
        Ok(board)
    }

    /// Nodos y flechas comparten los ids: una flecha nombra sus extremos por id.
    fn check_ids(&self) -> Result<(), FormatError> {
        let mut seen = HashSet::with_capacity(self.nodes.len() + self.edges.len());
        let ids = self.nodes.iter().map(|n| &n.id);
        if let Some(repeated) = ids
            .chain(self.edges.iter().map(|e| &e.id))
            .find(|id| !seen.insert(id.as_str()))
        {
            return Err(FormatError::DuplicateId(repeated.clone()));
        }
        let nodes: HashSet<&str> = self.nodes.iter().map(|n| n.id.as_str()).collect();
        for edge in &self.edges {
            for end in [&edge.from, &edge.to] {
                if !nodes.contains(end.node.as_str()) {
                    return Err(FormatError::DanglingEdge {
                        edge: edge.id.clone(),
                        node: end.node.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// El archivo canónico: un elemento por línea, claves en orden fijo y las
    /// desconocidas al final de su línea. Escribir dos veces da los mismos bytes.
    #[must_use]
    pub fn write(&self) -> String {
        let mut out = String::from("{\n\t\"nodes\":");
        write_lines(&mut out, self.nodes.iter().map(Node::line));
        out.push_str(",\n\t\"edges\":");
        write_lines(&mut out, self.edges.iter().map(Edge::line));
        for (key, value) in &self.extra {
            let _ = write!(out, ",\n\t{}:{value}", Value::from(key.as_str()));
        }
        out.push_str("\n}\n");
        out
    }

    #[must_use]
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|node| node.id == id)
    }

    #[must_use]
    pub fn edge(&self, id: &str) -> Option<&Edge> {
        self.edges.iter().find(|edge| edge.id == id)
    }

    /// Los nodos que caben enteros en `area`. Los grupos son fondo y no cuentan.
    #[must_use]
    pub fn nodes_within(&self, area: &Rect) -> Vec<String> {
        self.nodes
            .iter()
            .filter(|node| !matches!(node.kind, NodeKind::Group { .. }))
            .filter(|node| area.encloses(&node.rect()))
            .map(|node| node.id.clone())
            .collect()
    }
}

fn write_lines(out: &mut String, mut lines: impl Iterator<Item = String>) {
    let Some(first) = lines.next() else {
        out.push_str("[]");
        return;
    };
    let _ = write!(out, "[\n\t\t{first}");
    for line in lines {
        let _ = write!(out, ",\n\t\t{line}");
    }
    out.push_str("\n\t]");
}

/// Las claves que se escriben solo si traen algo, tras las conocidas.
pub(crate) fn write_extra(line: &mut String, extra: &Map<String, Value>) {
    for (key, value) in extra {
        let _ = write!(line, ",{}:{value}", Value::from(key.as_str()));
    }
}

#[cfg(test)]
#[path = "board_tests.rs"]
mod tests;
