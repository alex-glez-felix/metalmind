//! Dónde está cada clave del archivo, para ubicar las que se ignoraron.

use std::fmt;

use serde::Deserialize;
use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};

use super::Spanned;
use crate::diagnostic::Location;

/// El `Spanned` de serde-saphyr se ve como un newtype que envuelve
/// `{ <marca>: true, value: …, referenced: …, defined: … }`; sus claves no
/// son del usuario.
const SPANNED_MARKER: &str = "__serde_saphyr_private_spanned";

/// La ruta de una clave ignorada, o `None` si no es una clave del archivo.
pub(super) fn segments(path: &serde_ignored::Path<'_>) -> Option<Vec<String>> {
    use serde_ignored::Path;
    let (parent, key) = match path {
        Path::Root => return Some(Vec::new()),
        Path::Map { parent, key } if matches!(parent, Path::NewtypeStruct { .. }) => {
            return (key != SPANNED_MARKER).then(|| segments(parent)).flatten();
        }
        Path::Map { parent, key } => (parent, key.clone()),
        Path::Seq { parent, index } => (parent, index.to_string()),
        Path::Some { parent }
        | Path::NewtypeStruct { parent }
        | Path::NewtypeVariant { parent } => return segments(parent),
    };
    let mut keys = segments(parent)?;
    keys.push(key);
    Some(keys)
}

/// El documento reducido a sus claves y dónde se escribió cada una.
pub(super) enum Node {
    Map(Vec<(Spanned<String>, Node)>),
    Seq(Vec<Node>),
    Scalar,
}

impl Node {
    /// Dónde empieza la última clave de `path`.
    pub(super) fn key_location(&self, path: &[String]) -> Option<Location> {
        let (first, rest) = path.split_first()?;
        let (key, child) = match self {
            Self::Map(entries) => entries
                .iter()
                .find(|(key, _)| key.get_ref() == first)
                .map(|(key, child)| (key.location(), child))?,
            Self::Seq(items) => (None, items.get(first.parse::<usize>().ok()?)?),
            Self::Scalar => return None,
        };
        if rest.is_empty() {
            key
        } else {
            child.key_location(rest)
        }
    }
}

impl<'de> Deserialize<'de> for Node {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(NodeVisitor)
    }
}

struct NodeVisitor;

impl<'de> Visitor<'de> for NodeVisitor {
    type Value = Node;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("cualquier valor")
    }

    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Node, M::Error> {
        let mut entries = Vec::new();
        while let Some(key) = map.next_key()? {
            entries.push((key, map.next_value()?));
        }
        Ok(Node::Map(entries))
    }

    fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<Node, S::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(Node::Seq(items))
    }

    fn visit_bool<E: de::Error>(self, _: bool) -> Result<Node, E> {
        Ok(Node::Scalar)
    }

    fn visit_i64<E: de::Error>(self, _: i64) -> Result<Node, E> {
        Ok(Node::Scalar)
    }

    fn visit_i128<E: de::Error>(self, _: i128) -> Result<Node, E> {
        Ok(Node::Scalar)
    }

    fn visit_u64<E: de::Error>(self, _: u64) -> Result<Node, E> {
        Ok(Node::Scalar)
    }

    fn visit_u128<E: de::Error>(self, _: u128) -> Result<Node, E> {
        Ok(Node::Scalar)
    }

    fn visit_f64<E: de::Error>(self, _: f64) -> Result<Node, E> {
        Ok(Node::Scalar)
    }

    fn visit_str<E: de::Error>(self, _: &str) -> Result<Node, E> {
        Ok(Node::Scalar)
    }

    fn visit_bytes<E: de::Error>(self, _: &[u8]) -> Result<Node, E> {
        Ok(Node::Scalar)
    }

    fn visit_none<E: de::Error>(self) -> Result<Node, E> {
        Ok(Node::Scalar)
    }

    fn visit_unit<E: de::Error>(self) -> Result<Node, E> {
        Ok(Node::Scalar)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn locate(text: &str, path: &[&str]) -> Option<Location> {
        let path: Vec<String> = path.iter().map(|&key| key.to_owned()).collect();
        serde_saphyr::from_str::<Node>(text)
            .unwrap()
            .key_location(&path)
    }

    #[test]
    fn ubica_claves_anidadas_en_bloque_y_en_flujo() {
        let text = "a: 1\nb:\n  ñc: [1, { d: 2 }]\n  e: { f: x }\n";
        assert_eq!(locate(text, &["b"]), Some(Location { line: 2, column: 1 }));
        assert_eq!(
            locate(text, &["b", "ñc"]),
            Some(Location { line: 3, column: 3 })
        );
        assert_eq!(
            locate(text, &["b", "ñc", "1", "d"]),
            Some(Location {
                line: 3,
                column: 13
            })
        );
        assert_eq!(
            locate(text, &["b", "e", "f"]),
            Some(Location { line: 4, column: 8 })
        );
        assert_eq!(locate(text, &["b", "z"]), None);
        assert_eq!(locate(text, &["a", "z"]), None);
    }
}
