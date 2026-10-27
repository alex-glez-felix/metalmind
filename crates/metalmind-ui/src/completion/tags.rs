//! La lista del autocompletado de `#` (05 §3), sin GPUI: qué tags coinciden
//! con lo escrito, cuál está elegido y qué escribe aceptarlo.
//!
//! Se filtra con `nucleo` por el nombre y por los alias. Primero van los que
//! empiezan con lo escrito y, dentro de cada grupo, los de más notas: así
//! `#ru` ofrece `Rust` antes que `arquitectura` aunque esta tenga más.

use std::borrow::Cow;
use std::sync::Arc;

use metalmind_search::{FuzzyMatcher, Query};
use metalmind_syntax::TagPlace;
use metalmind_vault::tags::{TagCompletion, TagKey};

use super::Selection;

pub struct TagCompletions {
    /// Todos los tags del vault, de más notas a menos.
    candidates: Arc<[TagCompletion]>,
    /// Lo escrito del nombre, que aceptar reemplaza.
    typed: String,
    place: TagPlace,
    shown: Vec<Shown>,
    selection: Selection,
}

/// Un tag de la lista.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Shown {
    /// En `candidates`.
    index: usize,
    /// El alias con que coincidió, si no fue por el nombre.
    alias: Option<String>,
}

impl TagCompletions {
    /// La lista para `typed` en `place`; `None` si ningún tag coincide.
    /// Con `only_here`, lo escrito solo existe en el vault por esta misma
    /// ocurrencia (ya autoguardada) y no se ofrece a sí mismo.
    #[must_use]
    pub fn open(
        candidates: Arc<[TagCompletion]>,
        typed: &str,
        place: TagPlace,
        only_here: bool,
        matcher: &mut FuzzyMatcher,
    ) -> Option<Self> {
        let shown = matches(&candidates, typed, only_here, matcher);
        (!shown.is_empty()).then(|| Self {
            candidates,
            typed: typed.to_owned(),
            place,
            selection: Selection::new(shown.len()),
            shown,
        })
    }

    /// Si abrirla para `typed` en `place` entre `candidates` daría esta
    /// misma lista.
    #[must_use]
    pub fn is_for(&self, candidates: &Arc<[TagCompletion]>, typed: &str, place: TagPlace) -> bool {
        Arc::ptr_eq(&self.candidates, candidates) && self.is_typing(typed, place)
    }

    /// Si es la lista de `typed` en `place`, entre los tags que sean.
    #[must_use]
    pub fn is_typing(&self, typed: &str, place: TagPlace) -> bool {
        self.typed == typed && self.place == place
    }

    /// Esta lista entre los `candidates` de un lote nuevo del vault, con lo
    /// mismo escrito: el tag que eligió el usuario sigue elegido si todavía
    /// coincide; si no eligió, la primera fila.
    #[must_use]
    pub fn with_candidates(
        &self,
        candidates: Arc<[TagCompletion]>,
        only_here: bool,
        matcher: &mut FuzzyMatcher,
    ) -> Option<Self> {
        let mut list = Self::open(candidates, &self.typed, self.place, only_here, matcher)?;
        let Some(chosen) = self.selection.chosen() else {
            return Some(list);
        };
        let chosen = &self.candidates[self.shown[chosen].index].key;
        let row = list
            .rows()
            .position(|(completion, _)| completion.key == *chosen);
        if let Some(row) = row {
            list.selection.keep(row, &self.selection);
        }
        Some(list)
    }

    /// Cada tag de la lista con el alias por el que coincidió, en orden.
    pub fn rows(&self) -> impl ExactSizeIterator<Item = (&TagCompletion, Option<&str>)> {
        self.shown
            .iter()
            .map(|shown| (&self.candidates[shown.index], shown.alias.as_deref()))
    }

    /// Lo escrito del nombre.
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

    /// Lo que escribe aceptar en lugar de lo escrito: la forma establecida
    /// del tag elegido, aunque coincidiera por un alias (escribir `#rs` y
    /// aceptar deja `#Rust`, 05 §3).
    #[must_use]
    pub fn accept(&self) -> String {
        self.candidates[self.shown[self.selection.selected()].index].text(self.place)
    }
}

/// Por dónde coincide un tag, de mejor a peor: empezar con lo escrito gana
/// a coincidir en medio, y el nombre a un alias.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Side<'a> {
    in_middle: bool,
    alias: Option<&'a str>,
}

fn matches(
    candidates: &[TagCompletion],
    typed: &str,
    only_here: bool,
    matcher: &mut FuzzyMatcher,
) -> Vec<Shown> {
    let typed = TagKey::new(typed);
    let haystacks: Vec<(usize, Option<&str>)> = candidates
        .iter()
        .enumerate()
        .filter(|(_, completion)| !(only_here && completion.key == typed))
        .flat_map(|(index, completion)| {
            std::iter::once((index, None)).chain(
                completion
                    .aliases
                    .iter()
                    .map(move |alias| (index, Some(alias.as_str()))),
            )
        })
        .collect();
    // Plegado, como la identidad de un tag: `#RU` también encuentra `Rust`,
    // y `#groß`, `Gross`.
    let keys: Vec<Cow<'_, str>> = haystacks
        .iter()
        .map(|&(index, alias)| match alias {
            Some(alias) => Cow::Owned(TagKey::new(alias).as_str().to_owned()),
            None => Cow::Borrowed(candidates[index].key.as_str()),
        })
        .collect();
    let mut starts = vec![false; keys.len()];
    for ranked in matcher.rank(&Query::prefix(typed.as_str()), &keys) {
        starts[ranked.index] = true;
    }
    let mut best: Vec<Option<Side<'_>>> = vec![None; candidates.len()];
    for ranked in matcher.rank(&Query::new(typed.as_str()), &keys) {
        let (index, alias) = haystacks[ranked.index];
        let side = Side {
            in_middle: !starts[ranked.index],
            alias,
        };
        let better = |current: Side<'_>| {
            (side.in_middle, side.alias.is_some()) < (current.in_middle, current.alias.is_some())
        };
        if best[index].is_none_or(better) {
            best[index] = Some(side);
        }
    }
    let mut shown: Vec<(bool, Shown)> = best
        .into_iter()
        .enumerate()
        .filter_map(|(index, side)| {
            let side = side?;
            let alias = side.alias.map(str::to_owned);
            Some((side.in_middle, Shown { index, alias }))
        })
        .collect();
    shown.sort_by_key(|(in_middle, shown)| (*in_middle, shown.index));
    shown.into_iter().map(|(_, shown)| shown).collect()
}

#[cfg(test)]
#[path = "tags_tests.rs"]
mod tests;
