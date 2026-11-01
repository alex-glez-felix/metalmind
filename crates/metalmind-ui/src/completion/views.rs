//! La lista de vistas guardadas de `:focus @…` (06 §10), sin GPUI: cuáles
//! coinciden con lo escrito tras `@` y cuál está elegida.

use std::sync::Arc;

use metalmind_config::SavedView;
use metalmind_search::{FuzzyMatcher, Query};

use super::Selection;

pub struct ViewCompletions {
    views: Arc<[SavedView]>,
    typed: String,
    /// En `views`.
    shown: Vec<usize>,
    selection: Selection,
}

impl ViewCompletions {
    /// La lista para `typed`, en el orden del archivo si no hay nada
    /// escrito; `None` si ninguna vista coincide.
    #[must_use]
    pub fn open(views: Arc<[SavedView]>, typed: &str, matcher: &mut FuzzyMatcher) -> Option<Self> {
        let names: Vec<&str> = views.iter().map(|view| view.name.as_str()).collect();
        let shown: Vec<usize> = matcher
            .rank(&Query::literal(typed), &names)
            .into_iter()
            .map(|ranked| ranked.index)
            .collect();
        (!shown.is_empty()).then(|| Self {
            typed: typed.to_owned(),
            selection: Selection::new(shown.len()),
            shown,
            views,
        })
    }

    /// Si abrirla para `typed` entre `views` daría esta misma lista.
    #[must_use]
    pub fn is_for(&self, views: &Arc<[SavedView]>, typed: &str) -> bool {
        Arc::ptr_eq(&self.views, views) && self.typed == typed
    }

    /// Esta lista con otras vistas y lo mismo escrito: la que eligió el
    /// usuario sigue elegida si todavía coincide.
    #[must_use]
    pub fn with_views(&self, views: Arc<[SavedView]>, matcher: &mut FuzzyMatcher) -> Option<Self> {
        let mut list = Self::open(views, &self.typed, matcher)?;
        let Some(chosen) = self.selection.chosen() else {
            return Some(list);
        };
        let chosen = &self.views[self.shown[chosen]].name;
        let row = list.rows().position(|view| &view.name == chosen);
        if let Some(row) = row {
            list.selection.keep(row, &self.selection);
        }
        Some(list)
    }

    pub fn rows(&self) -> impl ExactSizeIterator<Item = &SavedView> {
        self.shown.iter().map(|&index| &self.views[index])
    }

    #[must_use]
    pub fn selection(&self) -> &Selection {
        &self.selection
    }

    pub fn selection_mut(&mut self) -> &mut Selection {
        &mut self.selection
    }

    /// La vista elegida.
    #[must_use]
    pub fn accept(&self) -> &SavedView {
        &self.views[self.shown[self.selection.selected()]]
    }
}
