//! La lista de carpetas del vault de un argumento de `:` (`:focus`), sin
//! GPUI: cuáles coinciden con lo escrito y cuál está elegida.
//!
//! Se filtra con `nucleo` por la ruta, tal cual: un espacio es parte del
//! nombre. Con nada escrito van en el orden del árbol.

use std::sync::Arc;

use metalmind_search::{FuzzyMatcher, Haystacks, Query};
use metalmind_vault::{EntryKind, VaultIndex, VaultPath};

use super::Selection;

/// Las carpetas de un índice del vault, ya convertidas para filtrar: se
/// arman una vez por lote, no en cada tecla.
pub struct FolderCandidates {
    index: Arc<VaultIndex>,
    folders: Vec<VaultPath>,
    haystacks: Haystacks,
}

impl FolderCandidates {
    #[must_use]
    pub fn new(index: Arc<VaultIndex>) -> Self {
        let mut folders: Vec<VaultPath> = index
            .iter()
            .filter(|entry| entry.kind == EntryKind::Folder)
            .map(|entry| entry.path.clone())
            .collect();
        folders.sort_unstable();
        let haystacks = Haystacks::new(folders.iter().map(VaultPath::as_str));
        Self {
            index,
            folders,
            haystacks,
        }
    }

    /// Si son las carpetas de `index`.
    #[must_use]
    pub fn is_of(&self, index: &Arc<VaultIndex>) -> bool {
        Arc::ptr_eq(&self.index, index)
    }
}

pub struct FolderCompletions {
    candidates: Arc<FolderCandidates>,
    typed: String,
    /// En los candidatos.
    shown: Vec<usize>,
    selection: Selection,
}

impl FolderCompletions {
    /// La lista para `typed`; `None` si ninguna carpeta coincide.
    #[must_use]
    pub fn open(
        candidates: Arc<FolderCandidates>,
        typed: &str,
        matcher: &mut FuzzyMatcher,
    ) -> Option<Self> {
        let all: Vec<usize> = (0..candidates.folders.len()).collect();
        let shown: Vec<usize> = matcher
            .rank_among(&Query::literal(typed), &candidates.haystacks, &all)
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
    pub fn is_for(&self, candidates: &Arc<FolderCandidates>, typed: &str) -> bool {
        Arc::ptr_eq(&self.candidates, candidates) && self.typed == typed
    }

    /// Esta lista entre los `candidates` de un lote nuevo del vault, con lo
    /// mismo escrito: la carpeta que eligió el usuario sigue elegida si
    /// todavía coincide; si no eligió, la primera fila.
    #[must_use]
    pub fn with_candidates(
        &self,
        candidates: Arc<FolderCandidates>,
        matcher: &mut FuzzyMatcher,
    ) -> Option<Self> {
        let mut list = Self::open(candidates, &self.typed, matcher)?;
        let Some(chosen) = self.selection.chosen() else {
            return Some(list);
        };
        let chosen = &self.candidates.folders[self.shown[chosen]];
        let row = list.rows().position(|folder| folder == chosen);
        if let Some(row) = row {
            list.selection.keep(row, &self.selection);
        }
        Some(list)
    }

    /// Las carpetas de la lista, en orden.
    pub fn rows(&self) -> impl ExactSizeIterator<Item = &VaultPath> {
        self.shown
            .iter()
            .map(|&index| &self.candidates.folders[index])
    }

    #[must_use]
    pub fn selection(&self) -> &Selection {
        &self.selection
    }

    pub fn selection_mut(&mut self) -> &mut Selection {
        &mut self.selection
    }

    /// La carpeta elegida.
    #[must_use]
    pub fn accept(&self) -> &VaultPath {
        &self.candidates.folders[self.shown[self.selection.selected()]]
    }
}
