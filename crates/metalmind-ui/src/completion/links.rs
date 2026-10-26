//! La lista del autocompletado de `[[`, sin GPUI: qué notas coinciden con
//! lo escrito del destino y cuál está elegida. El texto que escribe aceptar
//! lo calcula el índice de links (`LinkIndex::wikilink_target`).
//!
//! Son las notas del finder, en su orden: con nada escrito, las recientes
//! primero. Se filtra con `nucleo` por la ruta sin `.md`; primero van las
//! que tienen un nombre que empieza con lo escrito.

use std::sync::Arc;

use metalmind_search::{FuzzyMatcher, Query};
use metalmind_syntax::note_name;
use metalmind_vault::VaultPath;

use super::Selection;
use crate::finder::Note;

pub struct LinkCompletions {
    candidates: Arc<[Note]>,
    typed: String,
    /// En `candidates`.
    shown: Vec<usize>,
    selection: Selection,
}

impl LinkCompletions {
    /// La lista para `typed` desde la nota `source`, que no se ofrece a sí
    /// misma; `None` si ninguna otra coincide.
    #[must_use]
    pub fn open(
        candidates: Arc<[Note]>,
        typed: &str,
        source: &VaultPath,
        matcher: &mut FuzzyMatcher,
    ) -> Option<Self> {
        let shown = matches(&candidates, typed, source, matcher);
        (!shown.is_empty()).then(|| Self {
            candidates,
            typed: typed.to_owned(),
            selection: Selection::new(shown.len()),
            shown,
        })
    }

    /// Si abrirla para `typed` entre `candidates` daría esta misma lista.
    #[must_use]
    pub fn is_for(&self, candidates: &Arc<[Note]>, typed: &str) -> bool {
        Arc::ptr_eq(&self.candidates, candidates) && self.typed == typed
    }

    /// Esta lista entre los `candidates` de un lote nuevo del vault, con lo
    /// mismo escrito: la nota que eligió el usuario sigue elegida si todavía
    /// coincide; si no eligió, la primera fila.
    #[must_use]
    pub fn with_candidates(
        &self,
        candidates: Arc<[Note]>,
        source: &VaultPath,
        matcher: &mut FuzzyMatcher,
    ) -> Option<Self> {
        let mut list = Self::open(candidates, &self.typed, source, matcher)?;
        let Some(chosen) = self.selection.chosen() else {
            return Some(list);
        };
        let chosen = &self.candidates[self.shown[chosen]].0;
        let row = list.rows().position(|path| path == chosen);
        if let Some(row) = row {
            list.selection.keep(row, &self.selection);
        }
        Some(list)
    }

    /// Las notas de la lista, en orden.
    pub fn rows(&self) -> impl ExactSizeIterator<Item = &VaultPath> {
        self.shown.iter().map(|&index| &self.candidates[index].0)
    }

    /// Lo escrito del destino.
    #[must_use]
    pub fn typed(&self) -> &str {
        &self.typed
    }

    #[must_use]
    pub fn selection(&self) -> &Selection {
        &self.selection
    }

    pub fn selection_mut(&mut self) -> &mut Selection {
        &mut self.selection
    }

    /// La nota elegida.
    #[must_use]
    pub fn accept(&self) -> &VaultPath {
        &self.candidates[self.shown[self.selection.selected()]].0
    }
}

/// La ruta como se escribe en un wikilink, sin `.md`.
fn written(path: &str) -> &str {
    note_name::strip_extension(path).unwrap_or(path)
}

fn matches(
    candidates: &[Note],
    typed: &str,
    source: &VaultPath,
    matcher: &mut FuzzyMatcher,
) -> Vec<usize> {
    let paths: Vec<&str> = candidates
        .iter()
        .map(|note| written(note.0.as_str()))
        .collect();
    let names: Vec<&str> = candidates
        .iter()
        .map(|note| written(note.0.name()))
        .collect();
    let mut starts = vec![false; candidates.len()];
    for ranked in matcher.rank(&Query::prefix(typed), &names) {
        starts[ranked.index] = true;
    }
    let mut shown: Vec<usize> = matcher
        .rank(&Query::new(typed), &paths)
        .into_iter()
        .map(|ranked| ranked.index)
        .filter(|&index| candidates[index].0 != *source)
        .collect();
    shown.sort_by_key(|&index| !starts[index]);
    shown
}

#[cfg(test)]
#[path = "links_tests.rs"]
mod tests;
