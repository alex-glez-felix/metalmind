//! Lo que muestra la vista de tags del panel (05 §3): un grafo por
//! `parents` con el conteo de cada tag, sus notas y, al final, el buzón.

use std::collections::{HashMap, HashSet};

use crate::path::VaultPath;
use crate::tags::Tags;
use crate::tags::key::TagKey;
use crate::tags::page::{TagColor, TagIcon};
use crate::tags::scope::ScopeFilter;

/// La vista de tags de un momento, dentro de un alcance. Se arma de nuevo
/// con cada `VaultUpdate`; el panel recorre desde [`TagTree::roots`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TagTree {
    roots: Vec<TagKey>,
    nodes: HashMap<TagKey, TagNode>,
    inbox: Vec<VaultPath>,
}

/// Un tag del panel. Un tag con varios padres sale bajo cada uno; si ya está
/// en el camino desde la raíz, es un ciclo y el panel no lo expande otra vez.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagNode {
    pub key: TagKey,
    /// La forma establecida (05 §6).
    pub name: String,
    pub color: Option<TagColor>,
    pub icon: Option<TagIcon>,
    pub has_page: bool,
    /// Ni una nota ni una página lo nombran, solo algún `parents` (05 §7).
    pub only_parent: bool,
    /// Notas distintas del alcance con el tag, sus alias o sus descendientes.
    pub count: usize,
    /// Los tags que lo tienen de padre, en el orden del panel.
    pub children: Vec<TagKey>,
    /// Las notas del alcance que lo llevan escrito, o un alias suyo.
    pub notes: Vec<VaultPath>,
}

impl TagTree {
    /// Los tags de arriba, en el orden del panel.
    #[must_use]
    pub fn roots(&self) -> &[TagKey] {
        &self.roots
    }

    #[must_use]
    pub fn node(&self, key: &TagKey) -> Option<&TagNode> {
        self.nodes.get(key)
    }

    /// Todos los nodos, sin orden.
    pub fn nodes(&self) -> impl Iterator<Item = &TagNode> {
        self.nodes.values()
    }

    /// Las notas del alcance sin ningún tag (06 §9), en el orden del panel.
    #[must_use]
    pub fn inbox(&self) -> &[VaultPath] {
        &self.inbox
    }
}

impl Tags {
    /// La vista de tags dentro de `scope`. En focus solo quedan los tags con
    /// alguna nota del alcance; sin focus están todos, también los que solo
    /// tienen página o solo son padres.
    #[must_use]
    pub fn tree(&self, scope: &ScopeFilter<'_>) -> TagTree {
        let in_scope: HashSet<&VaultPath> = self
            .index
            .notes()
            .map(|(path, _)| path)
            .filter(|path| scope.contains(path))
            .collect();
        let mut nodes = HashMap::new();
        for key in self.node_keys() {
            let node = self.node(key, &in_scope);
            if scope.is_everything() || node.count > 0 {
                nodes.insert(key.clone(), node);
            }
        }
        let names: HashMap<TagKey, (String, String)> = nodes
            .iter()
            .map(|(key, node)| (key.clone(), name_order(&node.name)))
            .collect();
        let order = |keys: &mut Vec<TagKey>| {
            keys.retain(|key| names.contains_key(key));
            keys.sort_by(|a, b| names[a].cmp(&names[b]));
        };
        for node in nodes.values_mut() {
            order(&mut node.children);
        }
        let mut roots = self.roots();
        order(&mut roots);
        let mut inbox: Vec<VaultPath> = self
            .index
            .notes()
            .filter(|(path, note)| note.occurrences.is_empty() && in_scope.contains(path))
            .map(|(path, _)| path.clone())
            .collect();
        inbox.sort_by_cached_key(path_order);
        TagTree {
            roots,
            nodes,
            inbox,
        }
    }

    /// Los tags que son nodo del panel: escritos, con página o nombrados como
    /// padre. Los alias que aplican cuentan como su tag.
    pub(crate) fn node_keys(&self) -> impl Iterator<Item = &TagKey> {
        let mut keys: Vec<&TagKey> = self
            .index
            .keys()
            .chain(self.graph.pages().map(|page| &page.key))
            .chain(self.graph.parent_keys())
            .filter(|key| self.graph.alias_of(key).is_none())
            .collect();
        keys.sort_unstable();
        keys.dedup();
        keys.into_iter()
    }

    fn node(&self, key: &TagKey, scope: &HashSet<&VaultPath>) -> TagNode {
        let page = self.graph.page(key);
        let own: HashSet<&VaultPath> = std::iter::once(key)
            .chain(self.graph.aliases(key))
            .filter_map(|key| self.index.tag(key))
            .flat_map(|entry| &entry.notes)
            .filter(|path| scope.contains(path))
            .collect();
        let mut notes: Vec<VaultPath> = own.into_iter().cloned().collect();
        notes.sort_by_cached_key(path_order);
        TagNode {
            key: key.clone(),
            name: self.display_name(key).unwrap_or(key.as_str()).to_owned(),
            color: page.and_then(|page| page.color.clone()),
            icon: page.and_then(|page| page.icon.clone()),
            has_page: page.is_some(),
            only_parent: self.is_only_parent(key),
            count: self
                .notes_with(key)
                .into_iter()
                .filter(|path| scope.contains(path))
                .count(),
            children: self.graph.children(key).to_vec(),
            notes,
        }
    }
}

/// Alfabético sin distinguir mayúsculas, como el árbol de carpetas; a igual
/// nombre plegado, en orden de bytes para que no dependa del hash.
fn name_order(name: &str) -> (String, String) {
    (name.to_lowercase(), name.to_owned())
}

fn path_order(path: &VaultPath) -> (String, VaultPath) {
    (path.name().to_lowercase(), path.clone())
}
