//! Lo que un YAML dice, sin cómo está escrito: con él se comprueba que una
//! edición cambió justo lo pedido y nada más.

use std::fmt;

use serde::Deserialize;
use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Tree {
    Null,
    Bool(bool),
    Int(i128),
    Float(f64),
    Str(String),
    Seq(Vec<Tree>),
    Map(Vec<(Tree, Tree)>),
}

impl Tree {
    /// El valor de la clave `key` de un mapa.
    fn child_mut(&mut self, key: &str) -> Option<&mut Self> {
        let Self::Map(entries) = self else {
            return None;
        };
        entries
            .iter_mut()
            .find(|(k, _)| matches!(k, Self::Str(text) if text == key))
            .map(|(_, value)| value)
    }

    /// Como lo deja `edit::set`: las claves que faltan se agregan al final
    /// de su mapa, y un `null` en el camino pasa a ser un mapa.
    pub(super) fn set(&mut self, path: &[&str], value: Self) {
        let Some((first, rest)) = path.split_first() else {
            *self = value;
            return;
        };
        if *self == Self::Null {
            *self = Self::Map(Vec::new());
        }
        if self.child_mut(first).is_none()
            && let Self::Map(entries) = self
        {
            entries.push((Self::Str((*first).to_owned()), Self::Null));
        }
        if let Some(child) = self.child_mut(first) {
            if rest.is_empty() {
                *child = value;
            } else {
                child.set(rest, value);
            }
        }
    }

    /// Quita la clave `path`. Un mapa que se queda vacío, salvo el del
    /// documento, se queda sin valor: así queda su clave con solo comentarios.
    pub(super) fn remove(&mut self, path: &[&str]) {
        let Some((first, rest)) = path.split_first() else {
            return;
        };
        if let Some(child) = self.child_mut(first)
            && !rest.is_empty()
        {
            child.remove(rest);
            if rest.len() == 1 && *child == Self::Map(Vec::new()) {
                *child = Self::Null;
            }
            return;
        }
        if let Self::Map(entries) = self {
            entries.retain(|(k, _)| !matches!(k, Self::Str(text) if text == first));
        }
    }

    pub(super) fn rename(&mut self, path: &[&str], to: &str) {
        let Some((last, parents)) = path.split_last() else {
            return;
        };
        let mut node = self;
        for key in parents {
            let Some(child) = node.child_mut(key) else {
                return;
            };
            node = child;
        }
        if let Self::Map(entries) = node
            && let Some((key, _)) = entries
                .iter_mut()
                .find(|(k, _)| matches!(k, Self::Str(text) if text == last))
        {
            *key = Self::Str(to.to_owned());
        }
    }
}

impl<'de> Deserialize<'de> for Tree {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(TreeVisitor)
    }
}

struct TreeVisitor;

impl<'de> Visitor<'de> for TreeVisitor {
    type Value = Tree;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("un valor de YAML")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Tree, E> {
        Ok(Tree::Null)
    }

    fn visit_none<E: de::Error>(self) -> Result<Tree, E> {
        Ok(Tree::Null)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Tree, D::Error> {
        Tree::deserialize(deserializer)
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Tree, E> {
        Ok(Tree::Bool(value))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Tree, E> {
        Ok(Tree::Int(value.into()))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Tree, E> {
        Ok(Tree::Int(value.into()))
    }

    fn visit_i128<E: de::Error>(self, value: i128) -> Result<Tree, E> {
        Ok(Tree::Int(value))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Tree, E> {
        Ok(Tree::Float(value))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Tree, E> {
        Ok(Tree::Str(value.to_owned()))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Tree, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(Tree::Seq(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Tree, A::Error> {
        let mut entries = Vec::new();
        while let Some(entry) = map.next_entry()? {
            entries.push(entry);
        }
        Ok(Tree::Map(entries))
    }
}
