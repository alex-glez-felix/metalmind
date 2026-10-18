use std::ops::Range;

use smallvec::SmallVec;

use crate::ByteOffset;

/// Columna que los movimientos verticales intentan conservar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Goal {
    /// Se usa la columna actual del cursor.
    #[default]
    None,
    /// Columna en caracteres desde el inicio de la línea.
    Column(usize),
    /// Fin de línea, como tras `$` en vim.
    LineEnd,
}

/// Rango entre `anchor` (fijo) y `head` (donde está el cursor).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub anchor: ByteOffset,
    pub head: ByteOffset,
    pub goal: Goal,
}

impl Selection {
    #[must_use]
    pub const fn cursor(offset: ByteOffset) -> Self {
        Self {
            anchor: offset,
            head: offset,
            goal: Goal::None,
        }
    }

    #[must_use]
    pub const fn with_goal(self, goal: Goal) -> Self {
        Self { goal, ..self }
    }

    #[must_use]
    pub fn start(&self) -> ByteOffset {
        self.anchor.min(self.head)
    }

    #[must_use]
    pub fn end(&self) -> ByteOffset {
        self.anchor.max(self.head)
    }

    #[must_use]
    pub fn range(&self) -> Range<ByteOffset> {
        self.start()..self.end()
    }

    #[must_use]
    pub fn is_cursor(&self) -> bool {
        self.anchor == self.head
    }

    fn overlaps(&self, next: &Self) -> bool {
        next.start() < self.end() || (self.is_cursor() && next.start() == self.start())
    }

    fn merge(self, next: Self) -> Self {
        Self {
            anchor: self.start(),
            head: self.end().max(next.end()),
            goal: self.goal,
        }
    }
}

/// Conjunto no vacío de selecciones, ordenadas por inicio y sin solapes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selections(SmallVec<[Selection; 1]>);

impl Selections {
    #[must_use]
    pub fn single(selection: Selection) -> Self {
        Self(smallvec::smallvec![selection])
    }

    /// Ordena y fusiona; `None` si no hay ninguna selección.
    pub fn new(selections: impl IntoIterator<Item = Selection>) -> Option<Self> {
        let selections = normalized(selections.into_iter().collect());
        (!selections.is_empty()).then_some(Self(selections))
    }

    /// La primera selección del documento.
    #[must_use]
    pub fn primary(&self) -> &Selection {
        &self.0[0]
    }

    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &Selection> {
        self.0.iter()
    }

    /// Aplica `f` a cada selección y vuelve a ordenar y fusionar.
    #[must_use]
    pub fn map(&self, f: impl FnMut(&Selection) -> Selection) -> Self {
        Self(normalized(self.0.iter().map(f).collect()))
    }

    /// Un valor por selección, en el mismo orden.
    #[must_use]
    pub fn map_to<T>(&self, f: impl FnMut(&Selection) -> T) -> PerSelection<T> {
        PerSelection(self.0.iter().map(f).collect())
    }
}

/// Un valor por cada selección de un [`Selections`]. Solo se construye con
/// [`Selections::map_to`], así que nunca está vacío.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PerSelection<T>(SmallVec<[T; 1]>);

impl<T> PerSelection<T> {
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &T> {
        self.0.iter()
    }

    #[must_use]
    pub fn map<U>(&self, f: impl FnMut(&T) -> U) -> PerSelection<U> {
        PerSelection(self.0.iter().map(f).collect())
    }

    /// Vuelve a selecciones, ordenadas y fusionadas.
    #[must_use]
    pub fn to_selections(&self, f: impl FnMut(&T) -> Selection) -> Selections {
        Selections(normalized(self.0.iter().map(f).collect()))
    }
}

fn normalized(mut selections: SmallVec<[Selection; 1]>) -> SmallVec<[Selection; 1]> {
    selections.sort_by_key(Selection::start);
    let mut merged: SmallVec<[Selection; 1]> = SmallVec::with_capacity(selections.len());
    for selection in selections {
        match merged.last_mut() {
            Some(last) if last.overlaps(&selection) => *last = last.merge(selection),
            _ => merged.push(selection),
        }
    }
    merged
}

impl<'a> IntoIterator for &'a Selections {
    type Item = &'a Selection;
    type IntoIter = std::slice::Iter<'a, Selection>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cursor(at: usize) -> Selection {
        Selection::cursor(ByteOffset::new(at))
    }

    fn range(anchor: usize, head: usize) -> Selection {
        Selection {
            anchor: ByteOffset::new(anchor),
            head: ByteOffset::new(head),
            goal: Goal::None,
        }
    }

    #[test]
    fn sorts_and_merges_duplicated_cursors() {
        let selections = Selections::new([cursor(5), cursor(1), cursor(5)]).unwrap();
        let heads: Vec<_> = selections.iter().map(|s| s.head.get()).collect();
        assert_eq!(heads, [1, 5]);
    }

    #[test]
    fn merges_overlapping_ranges() {
        let selections = Selections::new([range(0, 4), range(6, 2)]).unwrap();
        assert_eq!(selections.iter().len(), 1);
        assert_eq!(
            selections.primary().range(),
            ByteOffset::new(0)..ByteOffset::new(6)
        );
    }

    #[test]
    fn keeps_touching_ranges_apart() {
        let selections = Selections::new([range(0, 2), range(2, 4)]).unwrap();
        assert_eq!(selections.iter().len(), 2);
    }

    #[test]
    fn per_selection_values_keep_their_order() {
        let selections = Selections::new([cursor(1), cursor(5)]).unwrap();
        let shifted = selections.map_to(|s| s.head.get() + 10);
        assert_eq!(shifted.iter().copied().collect::<Vec<_>>(), [11, 15]);
        let back = shifted.to_selections(|&at| cursor(at));
        let heads: Vec<_> = back.iter().map(|s| s.head.get()).collect();
        assert_eq!(heads, [11, 15]);
    }

    #[test]
    fn per_selection_results_are_normalized() {
        let selections = Selections::new([cursor(1), cursor(5)]).unwrap();
        let collapsed = selections.map_to(|_| ()).to_selections(|()| cursor(3));
        assert_eq!(collapsed.iter().len(), 1);
    }

    #[test]
    fn rejects_empty_set() {
        assert!(Selections::new([]).is_none());
    }
}
