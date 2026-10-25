//! Lo que un picker saca de lo escrito antes del fuzzy: en el finder, los
//! tags de `#rust -#archivado parser` dejan las notas que los cumplen y el
//! fuzzy busca `parser` entre ellas (05 §4).

use std::borrow::Cow;
use std::ops::Range;
use std::sync::Arc;

use metalmind_search::{FuzzyMatcher, Query, Ranked};

pub trait Narrowing<T>: Send + Sync {
    /// Lo escrito sin lo que filtra: el texto del fuzzy. Es barato; corre en
    /// el hilo de la UI.
    fn fuzzy<'t>(&self, text: &'t str) -> Cow<'t, str>;

    /// Si `text` filtra los ítems; si no, no hace falta llamar a
    /// [`Narrowing::keep`].
    fn filters(&self, text: &str) -> bool;

    /// Las posiciones de los ítems que deja pasar `text`, en orden. Corre en
    /// el fondo.
    fn keep(&self, text: &str, items: &[T]) -> Vec<usize>;

    /// Los bytes de `text` que se marcan como aviso en el input.
    fn warnings(&self, text: &str) -> Vec<Range<usize>>;
}

/// Lo que se le pide a un picker: lo escrito, y lo que lo estrecha si hay.
#[derive(Clone)]
pub struct Typed<T> {
    pub text: String,
    pub narrowing: Option<Arc<dyn Narrowing<T>>>,
}

impl<T> std::fmt::Debug for Typed<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Typed")
            .field("text", &self.text)
            .field("narrowing", &self.narrowing.is_some())
            .finish()
    }
}

impl<T: AsRef<str>> Typed<T> {
    /// El query del fuzzy.
    pub fn query(&self) -> Query {
        match &self.narrowing {
            Some(narrowing) => Query::new(&narrowing.fuzzy(&self.text)),
            None => Query::new(&self.text),
        }
    }

    /// Sin nada que buscar ni filtrar: se muestra todo sin correr nada.
    pub fn is_empty(&self) -> bool {
        self.query().is_empty() && !self.filters()
    }

    fn filters(&self) -> bool {
        self.narrowing
            .as_ref()
            .is_some_and(|narrowing| narrowing.filters(&self.text))
    }

    /// Los ítems que quedan, del mejor al peor; sin query, en su orden.
    pub fn rank(&self, query: &Query, items: &[T], matcher: &mut FuzzyMatcher) -> Vec<Ranked> {
        let kept = match &self.narrowing {
            Some(narrowing) if narrowing.filters(&self.text) => narrowing.keep(&self.text, items),
            _ if query.is_empty() => return in_order(0..items.len()),
            _ => return matcher.rank(query, items),
        };
        if query.is_empty() {
            return in_order(kept);
        }
        let candidates: Vec<&T> = kept.iter().map(|&index| &items[index]).collect();
        matcher
            .rank(query, &candidates)
            .into_iter()
            .map(|ranked| Ranked {
                index: kept[ranked.index],
                ..ranked
            })
            .collect()
    }
}

fn in_order(indices: impl IntoIterator<Item = usize>) -> Vec<Ranked> {
    indices
        .into_iter()
        .map(|index| Ranked { index, score: 0 })
        .collect()
}
