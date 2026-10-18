//! Los archivos del vault por clave de nombre, para resolver un destino sin
//! recorrer todo el vault (ver `nota_syntax::target_key`).

use std::collections::{BTreeSet, HashMap};
use std::ops::Bound;

use nota_syntax::{Resolution, path_keys, resolve_from, target_key};

use super::LinkTarget;
use crate::path::VaultPath;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Names {
    files: BTreeSet<VaultPath>,
    by_key: HashMap<String, BTreeSet<VaultPath>>,
}

impl Names {
    /// `path` y lo que hay dentro, si es una carpeta.
    pub fn within(&self, path: &VaultPath) -> Vec<VaultPath> {
        let inside = format!("{path}/");
        let mut found: Vec<VaultPath> = self
            .files
            .range::<str, _>((Bound::Included(inside.as_str()), Bound::Unbounded))
            .take_while(|file| file.as_str().starts_with(&inside))
            .cloned()
            .collect();
        if self.files.contains(path) {
            found.push(path.clone());
        }
        found
    }

    /// Las claves de `path`, que son las de los links a revisar al agregarlo o quitarlo.
    pub fn insert(&mut self, path: &VaultPath) -> Vec<String> {
        self.files.insert(path.clone());
        let keys: Vec<String> = path_keys(path.as_str()).collect();
        for key in &keys {
            self.by_key
                .entry(key.clone())
                .or_default()
                .insert(path.clone());
        }
        keys
    }

    pub fn remove(&mut self, path: &VaultPath) -> Vec<String> {
        if !self.files.remove(path) {
            return Vec::new();
        }
        let keys: Vec<String> = path_keys(path.as_str()).collect();
        for key in &keys {
            if let Some(paths) = self.by_key.get_mut(key) {
                paths.remove(path);
                if paths.is_empty() {
                    self.by_key.remove(key);
                }
            }
        }
        keys
    }

    pub fn candidates(&self, key: &str) -> impl Iterator<Item = &VaultPath> {
        self.by_key.get(key).into_iter().flatten()
    }

    pub fn resolve(&self, source: &VaultPath, target: &str) -> LinkTarget {
        resolve_among(source, target, |key| self.candidates(key).collect())
    }
}

/// Resuelve `target` escrito en `source` entre las rutas que da `candidates`
/// para la clave del destino, en orden: el de un empate.
pub(super) fn resolve_among<'p>(
    source: &VaultPath,
    target: &str,
    candidates: impl FnOnce(&str) -> Vec<&'p VaultPath>,
) -> LinkTarget {
    if target.trim().is_empty() {
        return LinkTarget::SameNote;
    }
    let Some(key) = target_key(source.as_str(), target) else {
        return LinkTarget::Unresolved;
    };
    let paths = candidates(&key);
    let owned = |found: &str| {
        paths
            .iter()
            .find(|path| path.as_str() == found)
            .map(|path| (*path).clone())
    };
    match resolve_from(source.as_str(), target, paths.iter().map(|p| p.as_str())) {
        Resolution::Resolved(found) => {
            owned(found).map_or(LinkTarget::Unresolved, LinkTarget::Resolved)
        }
        Resolution::Ambiguous(tied) => {
            LinkTarget::Ambiguous(tied.into_iter().filter_map(owned).collect())
        }
        Resolution::Unresolved => LinkTarget::Unresolved,
    }
}

/// La clave con que `target` busca candidatas; `None` en `[[#Heading]]` o
/// en un destino que no resuelve nunca.
pub(super) fn key_of(source: &VaultPath, target: &str) -> Option<String> {
    target_key(source.as_str(), target)
}
