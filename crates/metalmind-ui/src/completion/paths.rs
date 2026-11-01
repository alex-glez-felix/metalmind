//! La lista de rutas del vault de un argumento de `:`, sin GPUI: las
//! carpetas de `:focus` o los archivos de `:e` (#45), cuáles coinciden con
//! lo escrito y cuál está elegida.
//!
//! Se filtra con `nucleo` por la ruta, tal cual: un espacio es parte del
//! nombre. Con nada escrito van en orden alfabético.

use std::sync::Arc;

use metalmind_search::{FuzzyMatcher, Haystacks, Query};
use metalmind_vault::{EntryKind, VaultIndex, VaultPath};

use super::Selection;

/// Las carpetas o los archivos de un índice del vault, ya convertidos para
/// filtrar: se arman una vez por lote, no en cada tecla.
pub struct PathCandidates {
    index: Arc<VaultIndex>,
    paths: Vec<VaultPath>,
    haystacks: Haystacks,
}

impl PathCandidates {
    /// Las carpetas, para `:focus`.
    #[must_use]
    pub fn folders(index: Arc<VaultIndex>) -> Self {
        Self::new(index, |kind| kind == EntryKind::Folder)
    }

    /// Las notas y los adjuntos, para `:e`.
    #[must_use]
    pub fn files(index: Arc<VaultIndex>) -> Self {
        Self::new(index, |kind| kind != EntryKind::Folder)
    }

    fn new(index: Arc<VaultIndex>, wanted: impl Fn(EntryKind) -> bool) -> Self {
        let mut paths: Vec<VaultPath> = index
            .iter()
            .filter(|entry| wanted(entry.kind))
            .map(|entry| entry.path.clone())
            .collect();
        paths.sort_unstable();
        let haystacks = Haystacks::new(paths.iter().map(VaultPath::as_str));
        Self {
            index,
            paths,
            haystacks,
        }
    }

    /// Si son las rutas de `index`.
    #[must_use]
    pub fn is_of(&self, index: &Arc<VaultIndex>) -> bool {
        Arc::ptr_eq(&self.index, index)
    }
}

pub struct PathCompletions {
    candidates: Arc<PathCandidates>,
    typed: String,
    /// En los candidatos.
    shown: Vec<usize>,
    selection: Selection,
}

impl PathCompletions {
    /// La lista para `typed`; `None` si ninguna ruta coincide.
    #[must_use]
    pub fn open(
        candidates: Arc<PathCandidates>,
        typed: &str,
        matcher: &mut FuzzyMatcher,
    ) -> Option<Self> {
        let all: Vec<usize> = (0..candidates.paths.len()).collect();
        Self::among(candidates, &all, typed, matcher)
    }

    /// Como [`PathCompletions::open`], mirando solo los candidatos `pool`,
    /// en orden creciente.
    fn among(
        candidates: Arc<PathCandidates>,
        pool: &[usize],
        typed: &str,
        matcher: &mut FuzzyMatcher,
    ) -> Option<Self> {
        let shown: Vec<usize> = matcher
            .rank_among(&Query::literal(typed), &candidates.haystacks, pool)
            .into_iter()
            .map(|ranked| ranked.index)
            .collect();
        (!shown.is_empty()).then(|| Self {
            candidates,
            typed: typed.to_owned(),
            selection: Selection::new(shown.len()),
            shown,
        })
    }

    /// Si abrirla para `typed` entre `candidates` daría esta misma lista.
    #[must_use]
    pub fn is_for(&self, candidates: &Arc<PathCandidates>, typed: &str) -> bool {
        self.is_among(candidates) && self.typed == typed
    }

    /// Si es una lista entre `candidates`, con lo escrito que sea.
    #[must_use]
    pub fn is_among(&self, candidates: &Arc<PathCandidates>) -> bool {
        Arc::ptr_eq(&self.candidates, candidates)
    }

    #[must_use]
    pub fn typed(&self) -> &str {
        &self.typed
    }

    /// La lista con otro `typed` entre los mismos candidatos. Si sigue a lo
    /// escrito, solo filtra las rutas que ya coincidían: lo que no coincide
    /// con `ab` tampoco con `abc`.
    #[must_use]
    pub fn retyped(&self, typed: &str, matcher: &mut FuzzyMatcher) -> Option<Self> {
        let candidates = Arc::clone(&self.candidates);
        if !typed.starts_with(&self.typed) {
            return Self::open(candidates, typed, matcher);
        }
        let mut pool = self.shown.clone();
        pool.sort_unstable();
        Self::among(candidates, &pool, typed, matcher)
    }

    /// Esta lista entre los `candidates` de un lote nuevo del vault, con lo
    /// mismo escrito: la ruta que eligió el usuario sigue elegida si
    /// todavía coincide; si no eligió, la primera fila.
    #[must_use]
    pub fn with_candidates(
        &self,
        candidates: Arc<PathCandidates>,
        matcher: &mut FuzzyMatcher,
    ) -> Option<Self> {
        let mut list = Self::open(candidates, &self.typed, matcher)?;
        let Some(chosen) = self.selection.chosen() else {
            return Some(list);
        };
        let chosen = &self.candidates.paths[self.shown[chosen]];
        let row = list.rows().position(|path| path == chosen);
        if let Some(row) = row {
            list.selection.keep(row, &self.selection);
        }
        Some(list)
    }

    /// Las rutas de la lista, en orden.
    pub fn rows(&self) -> impl ExactSizeIterator<Item = &VaultPath> {
        self.shown
            .iter()
            .map(|&index| &self.candidates.paths[index])
    }

    #[must_use]
    pub fn selection(&self) -> &Selection {
        &self.selection
    }

    pub fn selection_mut(&mut self) -> &mut Selection {
        &mut self.selection
    }

    /// La ruta elegida.
    #[must_use]
    pub fn accept(&self) -> &VaultPath {
        &self.candidates.paths[self.shown[self.selection.selected()]]
    }
}
