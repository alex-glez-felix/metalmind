//! El delegado de los pickers que filtran una lista con la búsqueda difusa
//! de `metalmind-search`, como el finder y la paleta: [`PickerState`] corre
//! cada búsqueda en el executor de fondo de GPUI.

use std::sync::Arc;

use gpui_kit::{AppContext as _, Context, SharedString, Task};
use metalmind_search::FuzzyMatcher;

use super::highlight::match_ranges;
use super::view::{Overflow, PickerDelegate, PickerRow, PickerView};
use super::{Chosen, InputLine, LineEdit, Matches, Open, PickerState, Step};

/// Lo que se puede elegir en un picker difuso.
pub trait PickerItem: AsRef<str> + Send + Sync + 'static {
    /// Lo que va a la derecha de la fila, como el atajo de una acción.
    fn detail(&self) -> Option<&str> {
        None
    }
}

/// Qué clase de texto se busca: cambia los bonus del fuzzy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Haystack {
    Paths,
    Text,
}

impl Haystack {
    fn matcher(self) -> FuzzyMatcher {
        match self {
            Self::Paths => FuzzyMatcher::paths(),
            Self::Text => FuzzyMatcher::text(),
        }
    }

    fn overflow(self) -> Overflow {
        match self {
            Self::Paths => Overflow::Middle,
            Self::Text => Overflow::Around,
        }
    }
}

pub struct Fuzzy<T> {
    state: PickerState<T>,
    haystack: Haystack,
    /// Lo que dice la lista si no hay ningún ítem, como mientras se indexa.
    empty: SharedString,
    /// Se presta a la búsqueda en curso y vuelve con sus resultados.
    search_matcher: Option<FuzzyMatcher>,
    /// Para lo que corre en el hilo de la UI: las posiciones de las filas
    /// que se pintan y la búsqueda de un `enter` que no espera.
    ui_matcher: FuzzyMatcher,
    search: Option<Task<()>>,
}

impl<T: PickerItem> Fuzzy<T> {
    #[must_use]
    pub fn new(state: PickerState<T>, haystack: Haystack, empty: SharedString) -> Self {
        Self {
            state,
            haystack,
            empty,
            search_matcher: Some(haystack.matcher()),
            ui_matcher: haystack.matcher(),
            search: None,
        }
    }

    #[must_use]
    pub fn items(&self) -> &Arc<[T]> {
        self.state.items()
    }

    /// Cambia los ítems y vuelve a filtrar con lo escrito.
    pub fn set_items(&mut self, items: Arc<[T]>, cx: &mut Context<PickerView<Self>>) {
        let step = self.state.set_items(items);
        self.step(step, cx);
    }

    /// Corre la búsqueda que pidió el modelo y avisa si se eligió algo.
    fn step(&mut self, step: Step<T>, cx: &mut Context<PickerView<Self>>) {
        if let Some(job) = step.job {
            let haystack = self.haystack;
            let mut matcher = self
                .search_matcher
                .take()
                .unwrap_or_else(|| haystack.matcher());
            let running = cx.background_spawn(async move {
                let found = job.run(&mut matcher);
                (found, matcher)
            });
            self.search = Some(cx.spawn(async move |this, cx| {
                let (found, matcher) = running.await;
                // Si el picker ya se cerró, no hay nada que mostrar.
                let _ = this.update(cx, |this, cx| {
                    this.update_delegate(cx, |fuzzy, cx| fuzzy.finish(found, matcher, cx));
                });
            }));
        }
        if let Some(chosen) = step.chosen {
            cx.emit(super::PickerEvent::Chosen(chosen));
        }
    }

    fn finish(
        &mut self,
        found: Matches,
        matcher: FuzzyMatcher,
        cx: &mut Context<PickerView<Self>>,
    ) {
        self.search_matcher = Some(matcher);
        let step = self.state.finish(found);
        self.step(step, cx);
    }
}

impl<T: PickerItem> PickerDelegate for Fuzzy<T> {
    fn line(&self) -> &InputLine {
        self.state.line()
    }

    fn len(&self) -> usize {
        self.state.len()
    }

    fn selected(&self) -> usize {
        self.state.selected()
    }

    fn select(&mut self, row: usize) {
        self.state.select(row);
    }

    fn select_next(&mut self) {
        self.state.select_next();
    }

    fn select_previous(&mut self) {
        self.state.select_previous();
    }

    fn edit(&mut self, edit: LineEdit<'_>, cx: &mut Context<PickerView<Self>>) {
        let step = self.state.edit(edit);
        self.step(step, cx);
    }

    fn choose(&mut self, open: Open, _cx: &mut Context<PickerView<Self>>) -> Option<Chosen> {
        self.state.choose(open, &mut self.ui_matcher)
    }

    fn row(&mut self, row: usize) -> PickerRow {
        let positions = self.state.positions(row, &mut self.ui_matcher);
        let Some(item) = self.state.item(row) else {
            return PickerRow::default();
        };
        let text = item.as_ref().to_owned();
        PickerRow {
            highlights: match_ranges(&text, &positions),
            text,
            overflow: self.haystack.overflow(),
            detail: item.detail().map(str::to_owned),
        }
    }

    fn empty(&self) -> SharedString {
        if self.state.items().is_empty() {
            self.empty.clone()
        } else {
            SharedString::new_static("Sin resultados")
        }
    }
}
