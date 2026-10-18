//! Los archivos del vault por clave de nombre, para resolver un destino sin
//! recorrer todo el vault (ver `metalmind_syntax::target_key`). Un nombre solo
//! (`[[index]]`) se resuelve sin mirar cada candidata aunque el nombre se
//! repita en miles de carpetas: los desempates de `resolve_from` son
//! búsquedas en el grupo de las menos profundas.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ops::Bound;

use metalmind_syntax::{
    Resolution, TargetKey, path_keys, path_parent_key, resolve_from, target_key,
};

use super::LinkTarget;
use crate::path::VaultPath;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Names {
    files: BTreeSet<VaultPath>,
    by_key: HashMap<String, KeyFiles>,
}

/// Los archivos con una clave de nombre.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct KeyFiles {
    /// Por cantidad de carpetas.
    by_depth: BTreeMap<usize, Tied>,
    /// Por [`path_parent_key`], para los destinos con carpeta.
    by_parent: HashMap<String, BTreeSet<VaultPath>>,
}

/// Los archivos de una clave con la misma cantidad de carpetas: los que
/// empatan ante un nombre solo.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Tied {
    all: BTreeSet<VaultPath>,
    /// Por nombre exacto y, si es una nota, también sin `.md`.
    by_spelling: HashMap<String, BTreeSet<VaultPath>>,
    by_folder: HashMap<String, BTreeSet<VaultPath>>,
}

impl Names {
    pub fn contains(&self, path: &VaultPath) -> bool {
        self.files.contains(path)
    }

    /// `path` y lo que hay dentro, si es una carpeta; todo, si es la raíz.
    pub fn within(&self, path: &VaultPath) -> Vec<VaultPath> {
        if path.is_root() {
            return self.files.iter().cloned().collect();
        }
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
        if !self.files.insert(path.clone()) {
            return path_keys(path.as_str()).collect();
        }
        let keys: Vec<String> = path_keys(path.as_str()).collect();
        for key in &keys {
            self.by_key.entry(key.clone()).or_default().insert(path);
        }
        keys
    }

    pub fn remove(&mut self, path: &VaultPath) -> Vec<String> {
        if !self.files.remove(path) {
            return Vec::new();
        }
        let keys: Vec<String> = path_keys(path.as_str()).collect();
        for key in &keys {
            if let Some(files) = self.by_key.get_mut(key) {
                files.remove(path);
                if files.by_depth.is_empty() {
                    self.by_key.remove(key);
                }
            }
        }
        keys
    }

    pub fn resolve(&self, source: &VaultPath, target: &str) -> LinkTarget {
        resolve_with(|key| self.by_key.get(key), source, target)
    }

    /// Los nombres como quedan tras mover `moved` (las rutas de antes) de
    /// `from` a `to`. Solo copia las claves que cambian.
    pub fn renamed(&self, moved: &[VaultPath], from: &VaultPath, to: &VaultPath) -> Overlay<'_> {
        let mut changed: HashMap<String, KeyFiles> = HashMap::new();
        let mut added = Vec::new();
        for old in moved {
            let new = old.rebase(from, to).unwrap_or_else(|| old.clone());
            for key in path_keys(old.as_str()) {
                self.changed_entry(&mut changed, key).remove(old);
            }
            for key in path_keys(new.as_str()) {
                self.changed_entry(&mut changed, key.clone()).insert(&new);
                added.push(key);
            }
        }
        Overlay {
            base: self,
            changed,
            added,
        }
    }

    /// Los nombres con `path`, que puede no estar todavía. Solo copia sus claves.
    pub fn with_file(&self, path: &VaultPath) -> Overlay<'_> {
        let mut changed = HashMap::new();
        for key in path_keys(path.as_str()) {
            self.changed_entry(&mut changed, key).insert(path);
        }
        Overlay {
            base: self,
            changed,
            added: Vec::new(),
        }
    }

    fn changed_entry<'c>(
        &self,
        changed: &'c mut HashMap<String, KeyFiles>,
        key: String,
    ) -> &'c mut KeyFiles {
        let base = self.by_key.get(&key);
        changed
            .entry(key)
            .or_insert_with(|| base.cloned().unwrap_or_default())
    }
}

/// Los nombres con cambios que todavía no se hicieron: un rename
/// ([`Names::renamed`]) o un archivo recién creado ([`Names::with_file`]).
pub(super) struct Overlay<'a> {
    base: &'a Names,
    changed: HashMap<String, KeyFiles>,
    added: Vec<String>,
}

impl Overlay<'_> {
    pub fn resolve(&self, source: &VaultPath, target: &str) -> LinkTarget {
        let lookup = |key: &str| match self.changed.get(key) {
            Some(files) => (!files.by_depth.is_empty()).then_some(files),
            None => self.base.by_key.get(key),
        };
        resolve_with(lookup, source, target)
    }

    /// Las claves de las rutas nuevas.
    pub fn added_keys(&self) -> impl Iterator<Item = &String> {
        self.added.iter()
    }
}

fn resolve_with<'k>(
    lookup: impl Fn(&str) -> Option<&'k KeyFiles>,
    source: &VaultPath,
    target: &str,
) -> LinkTarget {
    if target.trim().is_empty() {
        return LinkTarget::SameNote;
    }
    let Some(key) = target_key(source.as_str(), target) else {
        return LinkTarget::Unresolved;
    };
    let Some(files) = lookup(&key.name) else {
        return LinkTarget::Unresolved;
    };
    match &key.parent {
        None => files.resolve_name(source, target.trim()),
        Some(parent) => {
            let candidates = files.by_parent.get(parent).into_iter().flatten();
            resolve_among(source, target, candidates)
        }
    }
}

impl KeyFiles {
    fn insert(&mut self, path: &VaultPath) {
        self.by_depth.entry(path.depth()).or_default().insert(path);
        add(&mut self.by_parent, path_parent_key(path.as_str()), path);
    }

    fn remove(&mut self, path: &VaultPath) {
        let depth = path.depth();
        if let Some(tied) = self.by_depth.get_mut(&depth) {
            tied.remove(path);
            if tied.all.is_empty() {
                self.by_depth.remove(&depth);
            }
        }
        remove(&mut self.by_parent, &path_parent_key(path.as_str()), path);
    }

    /// Un nombre solo, como `resolve_from`: gana la única de las menos
    /// profundas; si empatan, la única escrita igual y si no, la única en
    /// la carpeta de `source`.
    fn resolve_name(&self, source: &VaultPath, name: &str) -> LinkTarget {
        let Some(tied) = self.by_depth.values().next() else {
            return LinkTarget::Unresolved;
        };
        let folder = folder(source);
        let unique = |set: Option<&BTreeSet<VaultPath>>| match set.map(BTreeSet::len) {
            Some(1) => set.and_then(|set| set.first()).cloned(),
            _ => None,
        };
        unique(Some(&tied.all))
            .or_else(|| unique(tied.by_spelling.get(name)))
            .or_else(|| unique(tied.by_folder.get(folder)))
            .map_or_else(
                || LinkTarget::Ambiguous(tied.all.iter().cloned().collect()),
                LinkTarget::Resolved,
            )
    }
}

impl Tied {
    fn insert(&mut self, path: &VaultPath) {
        self.all.insert(path.clone());
        for spelling in spellings(path) {
            add(&mut self.by_spelling, spelling.to_owned(), path);
        }
        add(&mut self.by_folder, folder(path).to_owned(), path);
    }

    fn remove(&mut self, path: &VaultPath) {
        self.all.remove(path);
        for spelling in spellings(path) {
            remove(&mut self.by_spelling, spelling, path);
        }
        remove(&mut self.by_folder, folder(path), path);
    }
}

/// Cómo puede escribirse `path` en un nombre solo con sus mayúsculas: su
/// nombre y, si termina en `.md` (en cualquier caso), sin la extensión.
fn spellings(path: &VaultPath) -> impl Iterator<Item = &str> {
    let name = path.name();
    let stem = name
        .len()
        .checked_sub(".md".len())
        .filter(|&at| {
            name.get(at..)
                .is_some_and(|ext| ext.eq_ignore_ascii_case(".md"))
        })
        .map(|at| &name[..at]);
    std::iter::once(name).chain(stem)
}

fn folder(path: &VaultPath) -> &str {
    path.as_str()
        .rsplit_once('/')
        .map_or("", |(folder, _)| folder)
}

fn add(map: &mut HashMap<String, BTreeSet<VaultPath>>, key: String, path: &VaultPath) {
    map.entry(key).or_default().insert(path.clone());
}

fn remove(map: &mut HashMap<String, BTreeSet<VaultPath>>, key: &str, path: &VaultPath) {
    if let Some(set) = map.get_mut(key) {
        set.remove(path);
        if set.is_empty() {
            map.remove(key);
        }
    }
}

/// Resuelve `target` escrito en `source` entre `candidates`, que tienen
/// que incluir toda ruta con las claves del destino; el orden es el de un
/// empate.
fn resolve_among<'p>(
    source: &VaultPath,
    target: &str,
    candidates: impl Iterator<Item = &'p VaultPath>,
) -> LinkTarget {
    let paths: Vec<&VaultPath> = candidates.collect();
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
    target_key(source.as_str(), target).map(|TargetKey { name, .. }| name)
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn path() -> impl Strategy<Value = String> {
        let part = prop::sample::select(vec![
            "a", "A", "b", "x.md", "X.md", "x.MD", "x", "y.png", "é",
        ]);
        prop::collection::vec(part, 1..=4).prop_map(|parts| parts.join("/"))
    }

    fn target() -> impl Strategy<Value = String> {
        let part = prop::sample::select(vec!["a", "A", "b", "x", "X", "x.md", "X.MD", "é", ".."]);
        (
            prop::sample::select(vec!["", "/", "./", "../", " "]),
            prop::collection::vec(part, 1..=3),
        )
            .prop_map(|(prefix, parts)| format!("{prefix}{}", parts.join("/")))
    }

    proptest! {
        /// Con los grupos y los desempates por búsqueda, resolver da lo mismo
        /// que pasarle todo el vault a `resolve_from`.
        #[test]
        fn resolves_like_resolve_from_over_the_whole_vault(
            files in prop::collection::btree_set(path(), 0..16),
            removed in prop::collection::btree_set(path(), 0..4),
            source in path(),
            target in target(),
        ) {
            let valid = |paths: &BTreeSet<String>| -> BTreeSet<VaultPath> {
                paths.iter().filter_map(|path| VaultPath::new(path).ok()).collect()
            };
            let (files, removed) = (valid(&files), valid(&removed));
            let mut names = Names::default();
            for file in files.union(&removed) {
                names.insert(file);
            }
            for file in removed.difference(&files) {
                names.remove(file);
            }
            let source = VaultPath::new(&source).unwrap();
            prop_assert_eq!(
                names.resolve(&source, &target),
                resolve_among(&source, &target, files.iter())
            );
        }
    }
}
