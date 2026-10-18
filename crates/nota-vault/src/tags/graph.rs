//! Lo que significan los tags según sus páginas: alias, padres y los
//! problemas de 05 §7. Sale solo de las páginas, así que rehacerlo no obliga
//! a reescanear ninguna nota.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::path::VaultPath;
use crate::tags::cycles::cycles;
use crate::tags::key::TagKey;
use crate::tags::page::{InvalidValue, TagPage};

/// Algo de las páginas de tag que hay que avisar. Nada de esto impide usar
/// el resto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagProblem {
    /// `tags/mi tag.md`: no existe un `#tag` que lo nombre; es una nota normal.
    InvalidPageName { path: VaultPath },
    /// Dos archivos con la misma clave: vale el primero en orden de bytes.
    DuplicatePage { kept: VaultPath, ignored: VaultPath },
    /// Un valor de la página que no se entiende.
    InvalidValue {
        page: VaultPath,
        value: InvalidValue,
    },
    /// El alias es el nombre de otro tag con página: no aplica.
    AliasIsPage {
        page: VaultPath,
        alias: TagKey,
        other: VaultPath,
    },
    /// Varias páginas declaran el mismo alias: no aplica en ninguna.
    AliasConflict {
        alias: TagKey,
        pages: Vec<VaultPath>,
    },
    /// La página se pone a sí misma como padre: ese padre se ignora.
    SelfParent { page: VaultPath },
    /// Un ciclo de padres, con el primer tag repetido al final (`a → b → a`).
    Cycle { path: Vec<TagKey> },
}

/// El grafo de las páginas de tag.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TagGraph {
    pages: BTreeMap<TagKey, TagPage>,
    alias_of: BTreeMap<TagKey, TagKey>,
    aliases: BTreeMap<TagKey, Vec<TagKey>>,
    parents: BTreeMap<TagKey, Vec<TagKey>>,
    children: BTreeMap<TagKey, Vec<TagKey>>,
    problems: Vec<TagProblem>,
}

impl TagGraph {
    /// Arma el grafo. `pages` va en orden de ruta, que decide los empates.
    pub(crate) fn build<'p>(
        pages: impl IntoIterator<Item = &'p TagPage>,
        invalid_names: impl IntoIterator<Item = &'p VaultPath>,
    ) -> Self {
        let mut graph = Self::default();
        graph.problems.extend(
            invalid_names
                .into_iter()
                .map(|path| TagProblem::InvalidPageName { path: path.clone() }),
        );
        for page in pages {
            graph.add_page(page);
        }
        graph.resolve_aliases();
        graph.resolve_parents();
        graph.problems.extend(
            cycles(&graph.parents)
                .into_iter()
                .map(|path| TagProblem::Cycle { path }),
        );
        graph
    }

    fn add_page(&mut self, page: &TagPage) {
        if let Some(kept) = self.pages.get(&page.key) {
            self.problems.push(TagProblem::DuplicatePage {
                kept: kept.path.clone(),
                ignored: page.path.clone(),
            });
            return;
        }
        self.problems
            .extend(page.invalid.iter().map(|value| TagProblem::InvalidValue {
                page: page.path.clone(),
                value: value.clone(),
            }));
        self.pages.insert(page.key.clone(), page.clone());
    }

    fn resolve_aliases(&mut self) {
        let mut declared: BTreeMap<&TagKey, BTreeSet<&TagKey>> = BTreeMap::new();
        for page in self.pages.values() {
            for alias in &page.aliases {
                if alias.key == page.key {
                    continue;
                }
                if let Some(other) = self.pages.get(&alias.key) {
                    self.problems.push(TagProblem::AliasIsPage {
                        page: page.path.clone(),
                        alias: alias.key.clone(),
                        other: other.path.clone(),
                    });
                    continue;
                }
                declared.entry(&alias.key).or_default().insert(&page.key);
            }
        }
        let mut alias_of = BTreeMap::new();
        let mut problems = Vec::new();
        for (alias, owners) in declared {
            match owners.first() {
                Some(owner) if owners.len() == 1 => {
                    alias_of.insert(alias.clone(), (*owner).clone());
                }
                _ => problems.push(TagProblem::AliasConflict {
                    alias: alias.clone(),
                    pages: owners
                        .iter()
                        .filter_map(|key| self.pages.get(*key))
                        .map(|page| page.path.clone())
                        .collect(),
                }),
            }
        }
        for (alias, owner) in &alias_of {
            self.aliases
                .entry(owner.clone())
                .or_default()
                .push(alias.clone());
        }
        self.alias_of = alias_of;
        self.problems.extend(problems);
    }

    fn resolve_parents(&mut self) {
        let mut parents = BTreeMap::new();
        for page in self.pages.values() {
            let mut resolved: Vec<TagKey> = Vec::new();
            let mut itself = false;
            for parent in &page.parents {
                let parent = self.resolve(&parent.key).clone();
                if parent == page.key {
                    itself = true;
                } else if !resolved.contains(&parent) {
                    resolved.push(parent);
                }
            }
            if itself {
                self.problems.push(TagProblem::SelfParent {
                    page: page.path.clone(),
                });
            }
            if !resolved.is_empty() {
                parents.insert(page.key.clone(), resolved);
            }
        }
        for (child, of) in &parents {
            for parent in of {
                self.children
                    .entry(parent.clone())
                    .or_default()
                    .push(child.clone());
            }
        }
        self.parents = parents;
    }

    /// El tag que nombra `key`: el dueño si es un alias que aplica, si no él mismo.
    #[must_use]
    pub fn resolve<'k>(&'k self, key: &'k TagKey) -> &'k TagKey {
        self.alias_of.get(key).unwrap_or(key)
    }

    /// El tag del que `key` es alias, si lo es y aplica.
    #[must_use]
    pub fn alias_of(&self, key: &TagKey) -> Option<&TagKey> {
        self.alias_of.get(key)
    }

    #[must_use]
    pub fn page(&self, key: &TagKey) -> Option<&TagPage> {
        self.pages.get(key)
    }

    /// Las páginas que valen, en orden de clave.
    pub fn pages(&self) -> impl Iterator<Item = &TagPage> {
        self.pages.values()
    }

    /// Los alias que aplican de `key`, en orden.
    #[must_use]
    pub fn aliases(&self, key: &TagKey) -> &[TagKey] {
        self.aliases.get(key).map_or(&[], Vec::as_slice)
    }

    /// Los padres de `key`, ya resueltos, en el orden de su página.
    #[must_use]
    pub fn parents(&self, key: &TagKey) -> &[TagKey] {
        self.parents.get(key).map_or(&[], Vec::as_slice)
    }

    /// Los tags que tienen a `key` como padre, en orden de clave.
    #[must_use]
    pub fn children(&self, key: &TagKey) -> &[TagKey] {
        self.children.get(key).map_or(&[], Vec::as_slice)
    }

    /// Los tags que algún `parents` nombra, existan o no como página.
    pub fn parent_keys(&self) -> impl Iterator<Item = &TagKey> {
        self.children.keys()
    }

    #[must_use]
    pub fn problems(&self) -> &[TagProblem] {
        &self.problems
    }

    /// `key` y todos sus descendientes por `parents`, sin dar vueltas en un ciclo.
    #[must_use]
    pub fn descendants(&self, key: &TagKey) -> BTreeSet<TagKey> {
        let mut seen = BTreeSet::from([key.clone()]);
        let mut pending = VecDeque::from([key]);
        while let Some(next) = pending.pop_front() {
            for child in self.children(next) {
                if seen.insert(child.clone()) {
                    pending.push_back(child);
                }
            }
        }
        seen
    }

    /// `key` y todos sus ancestros por `parents`, sin dar vueltas en un ciclo.
    #[must_use]
    pub fn ancestors<'k>(&'k self, key: &'k TagKey) -> Vec<&'k TagKey> {
        let mut seen = vec![key];
        let mut pending = VecDeque::from([key]);
        while let Some(next) = pending.pop_front() {
            for parent in self.parents(next) {
                if !seen.contains(&parent) {
                    seen.push(parent);
                    pending.push_back(parent);
                }
            }
        }
        seen
    }

    /// Todas las claves con las que se escribe `key` o un descendiente: ellos
    /// y sus alias. Es lo que trae `#key` en una consulta.
    #[must_use]
    pub fn expansion(&self, key: &TagKey) -> BTreeSet<TagKey> {
        let descendants = self.descendants(self.resolve(key));
        let aliases: Vec<TagKey> = descendants
            .iter()
            .flat_map(|tag| self.aliases(tag))
            .cloned()
            .collect();
        descendants.into_iter().chain(aliases).collect()
    }
}
