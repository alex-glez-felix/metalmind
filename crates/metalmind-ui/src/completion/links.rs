//! La lista del autocompletado de `[[`, sin GPUI: qué notas coinciden con
//! lo escrito del destino y cuál está elegida. El texto que escribe aceptar
//! lo calcula el índice de links (`LinkIndex::wikilink_target`).
//!
//! Son las notas del finder, en su orden: con nada escrito, las recientes
//! primero. Se filtra con `nucleo` por la ruta sin `.md`; primero van las
//! que tienen un nombre que empieza con lo escrito.

use std::sync::Arc;

use metalmind_search::{FuzzyMatcher, Haystacks, Query};
use metalmind_syntax::note_name;
use metalmind_vault::VaultPath;

use super::Selection;
use crate::finder::Note;

/// Las notas que ofrece la lista, con sus rutas y sus nombres sin `.md` ya
/// convertidos para filtrar: se arman una vez por lote del vault, no en
/// cada tecla.
pub struct NoteCandidates {
    notes: Arc<[Note]>,
    paths: Haystacks,
    names: Haystacks,
}

impl NoteCandidates {
    #[must_use]
    pub fn new(notes: Arc<[Note]>) -> Self {
        let paths = Haystacks::new(notes.iter().map(|note| written(note.0.as_str())));
        let names = Haystacks::new(notes.iter().map(|note| written(note.0.name())));
        Self {
            notes,
            paths,
            names,
        }
    }

    /// Si son estas mismas notas, en el mismo orden: las mismas casi
    /// siempre, y si no, las de un lote que no cambió ninguna.
    #[must_use]
    pub fn has(&self, notes: &Arc<[Note]>) -> bool {
        Arc::ptr_eq(&self.notes, notes) || *self.notes == **notes
    }
}

pub struct LinkCompletions {
    candidates: Arc<NoteCandidates>,
    /// La nota donde se escribe el link, que no se ofrece a sí misma.
    source: VaultPath,
    typed: String,
    /// En los candidatos.
    shown: Vec<usize>,
    selection: Selection,
}

impl LinkCompletions {
    /// La lista para `typed` desde la nota `source`; `None` si ninguna otra
    /// coincide.
    #[must_use]
    pub fn open(
        candidates: Arc<NoteCandidates>,
        typed: &str,
        source: &VaultPath,
        matcher: &mut FuzzyMatcher,
    ) -> Option<Self> {
        let all: Vec<usize> = (0..candidates.notes.len()).collect();
        Self::among(candidates, &all, typed, source, matcher)
    }

    /// Como [`LinkCompletions::open`], mirando solo los candidatos `pool`,
    /// en orden creciente.
    fn among(
        candidates: Arc<NoteCandidates>,
        pool: &[usize],
        typed: &str,
        source: &VaultPath,
        matcher: &mut FuzzyMatcher,
    ) -> Option<Self> {
        let shown = matches(&candidates, pool, typed, source, matcher);
        (!shown.is_empty()).then(|| Self {
            candidates,
            source: source.clone(),
            typed: typed.to_owned(),
            selection: Selection::new(shown.len()),
            shown,
        })
    }

    /// Si abrirla para `typed` desde `source` entre `candidates` daría esta
    /// misma lista.
    #[must_use]
    pub fn is_for(
        &self,
        candidates: &Arc<NoteCandidates>,
        typed: &str,
        source: &VaultPath,
    ) -> bool {
        self.is_among(candidates, source) && self.typed == typed
    }

    /// Si es una lista desde `source` entre `candidates`, con lo escrito que
    /// sea.
    #[must_use]
    pub fn is_among(&self, candidates: &Arc<NoteCandidates>, source: &VaultPath) -> bool {
        Arc::ptr_eq(&self.candidates, candidates) && self.source == *source
    }

    /// La lista con otro `typed` entre los mismos candidatos. Si sigue a lo
    /// escrito, solo filtra las que ya coincidían: lo que no coincide con
    /// `ab` tampoco con `abc`.
    #[must_use]
    pub fn retyped(&self, typed: &str, matcher: &mut FuzzyMatcher) -> Option<Self> {
        let candidates = Arc::clone(&self.candidates);
        if !typed.starts_with(&self.typed) {
            return Self::open(candidates, typed, &self.source, matcher);
        }
        let mut pool = self.shown.clone();
        pool.sort_unstable();
        Self::among(candidates, &pool, typed, &self.source, matcher)
    }

    /// Esta lista entre los `candidates` de un lote nuevo del vault, con lo
    /// mismo escrito: la nota que eligió el usuario sigue elegida si todavía
    /// coincide; si no eligió, la primera fila.
    #[must_use]
    pub fn with_candidates(
        &self,
        candidates: Arc<NoteCandidates>,
        matcher: &mut FuzzyMatcher,
    ) -> Option<Self> {
        let mut list = Self::open(candidates, &self.typed, &self.source, matcher)?;
        let Some(chosen) = self.selection.chosen() else {
            return Some(list);
        };
        let chosen = &self.candidates.notes[self.shown[chosen]].0;
        let row = list.rows().position(|path| path == chosen);
        if let Some(row) = row {
            list.selection.keep(row, &self.selection);
        }
        Some(list)
    }

    /// La nota donde se escribe el link.
    #[must_use]
    pub fn source(&self) -> &VaultPath {
        &self.source
    }

    /// Las notas de la lista, en orden.
    pub fn rows(&self) -> impl ExactSizeIterator<Item = &VaultPath> {
        self.shown
            .iter()
            .map(|&index| &self.candidates.notes[index].0)
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
        &self.candidates.notes[self.shown[self.selection.selected()]].0
    }
}

/// La ruta como se escribe en un wikilink, sin `.md`.
fn written(path: &str) -> &str {
    note_name::strip_extension(path).unwrap_or(path)
}

/// Las de `pool` que coinciden con `typed`, sin `source`: primero las que
/// tienen un nombre que empieza con lo escrito y, en cada grupo, de mejor a
/// peor coincidencia por la ruta. Lo escrito se busca tal cual: un `!` o un
/// `^` en un nombre no son operadores.
fn matches(
    candidates: &NoteCandidates,
    pool: &[usize],
    typed: &str,
    source: &VaultPath,
    matcher: &mut FuzzyMatcher,
) -> Vec<usize> {
    let mut starts = vec![false; candidates.notes.len()];
    let paths = matcher.rank_among(&Query::literal(typed), &candidates.paths, pool);
    let found: Vec<usize> = paths.iter().map(|ranked| ranked.index).collect();
    let mut in_order = found.clone();
    in_order.sort_unstable();
    for ranked in matcher.rank_among(&Query::prefix(typed), &candidates.names, &in_order) {
        starts[ranked.index] = true;
    }
    let mut shown: Vec<usize> = found
        .into_iter()
        .filter(|&index| candidates.notes[index].0 != *source)
        .collect();
    shown.sort_by_key(|&index| !starts[index]);
    shown
}

#[cfg(test)]
#[path = "links_tests.rs"]
mod tests;
