//! Cuántas notas trae cada tag y cuáles no tienen ninguno, en una sola
//! pasada por las notas: cada nota suma una vez a sus tags y a todos sus
//! ancestros por `parents`, que es el conteo de notas distintas de 05 §2.

use std::collections::HashMap;

use crate::path::VaultPath;
use crate::tags::Tags;
use crate::tags::key::TagKey;

/// Los nodos del panel con su conteo. Cada tag tiene un número, para que
/// sumar a los ancestros no vuelva a hashear claves.
#[derive(Debug)]
pub(crate) struct Tally<'t> {
    /// Los nodos del panel en orden de clave ([`Tags::node_keys`]).
    pub keys: Vec<&'t TagKey>,
    ids: HashMap<&'t TagKey, usize>,
    /// Notas distintas con el tag, sus alias o sus descendientes.
    counts: Vec<usize>,
    /// Las notas sin tags.
    pub inbox: Vec<&'t VaultPath>,
}

impl Tally<'_> {
    pub fn count(&self, key: &TagKey) -> usize {
        self.ids.get(key).map_or(0, |&id| self.counts[id])
    }
}

impl Tags {
    /// Cuenta las notas (nunca las páginas) para las que `in_scope` dice que sí.
    pub(crate) fn tally(&self, in_scope: impl Fn(&VaultPath) -> bool) -> Tally<'_> {
        let keys: Vec<&TagKey> = self.node_keys().collect();
        let ids: HashMap<&TagKey, usize> = keys
            .iter()
            .enumerate()
            .map(|(id, &key)| (key, id))
            .collect();
        let mut tally = Tally {
            counts: vec![0; keys.len()],
            inbox: Vec::new(),
            keys,
            ids,
        };
        // Cada clave escrita se resuelve una vez, no en cada ocurrencia.
        let written_ids: HashMap<&TagKey, usize> = self
            .index
            .keys()
            .filter_map(|key| Some((key, *tally.ids.get(self.graph.resolve(key))?)))
            .collect();
        let mut ancestry: Vec<Option<Vec<usize>>> = vec![None; tally.keys.len()];
        let mut written: Vec<usize> = Vec::new();
        let mut lifted: Vec<usize> = Vec::new();
        for (path, note) in self.index.notes() {
            if !in_scope(path) {
                continue;
            }
            written.clear();
            written.extend(
                note.occurrences
                    .iter()
                    .filter_map(|occurrence| written_ids.get(&occurrence.key))
                    .copied(),
            );
            written.sort_unstable();
            written.dedup();
            if written.is_empty() {
                tally.inbox.push(path);
                continue;
            }
            lifted.clear();
            for &id in &written {
                let ancestors = ancestry[id].get_or_insert_with(|| {
                    self.graph
                        .ancestors(tally.keys[id])
                        .into_iter()
                        .filter_map(|key| tally.ids.get(key).copied())
                        .collect()
                });
                lifted.extend_from_slice(ancestors);
            }
            lifted.sort_unstable();
            lifted.dedup();
            for &id in &lifted {
                tally.counts[id] += 1;
            }
        }
        tally
    }
}
