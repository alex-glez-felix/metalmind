//! El filtro del árbol: la búsqueda difusa de rutas de `metalmind-search` sobre
//! todo el vault, con las carpetas que contienen cada coincidencia.

use std::collections::{HashMap, HashSet};

use metalmind_search::{FuzzyMatcher, Query};
use metalmind_vault::{VaultIndex, VaultPath};

pub(super) struct Matches {
    /// Lo que se ve, con las letras de su nombre que coinciden; las
    /// carpetas que solo contienen coincidencias no tienen ninguna.
    visible: HashMap<VaultPath, Vec<usize>>,
    /// Las carpetas con alguna coincidencia dentro.
    parents: HashSet<VaultPath>,
}

impl Matches {
    pub(super) fn find(index: &VaultIndex, query: &str) -> Self {
        let paths: Vec<&VaultPath> = index.iter().map(|entry| entry.path).collect();
        let candidates: Vec<&str> = paths.iter().map(|path| path.as_str()).collect();
        let query = Query::new(query);
        let mut fuzzy = FuzzyMatcher::paths();
        let found = fuzzy.rank(&query, &candidates);
        let mut matches = Self {
            visible: HashMap::with_capacity(found.len()),
            parents: HashSet::new(),
        };
        for found in found {
            let path = paths[found.index];
            let name_start = path.as_str().len() - path.name().len();
            let highlights = fuzzy
                .positions(&query, path.as_str())
                .iter()
                .filter_map(|position| position.checked_sub(name_start))
                .collect();
            matches.visible.insert(path.clone(), highlights);
            for ancestor in path.ancestors().filter(|ancestor| !ancestor.is_root()) {
                if !matches.parents.insert(ancestor.clone()) {
                    break;
                }
                matches.visible.entry(ancestor).or_default();
            }
        }
        matches
    }

    pub(super) fn get(&self, path: &VaultPath) -> Option<&[usize]> {
        self.visible.get(path).map(Vec::as_slice)
    }

    pub(super) fn has_inside(&self, folder: &VaultPath) -> bool {
        self.parents.contains(folder)
    }
}
