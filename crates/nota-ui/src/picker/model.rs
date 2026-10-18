//! El estado de un picker, sin GPUI: lo escrito, lo que coincide, la fila
//! elegida y qué búsqueda falta correr.
//!
//! Filtrar corre fuera del hilo de la UI, de a una búsqueda a la vez, con
//! la última consulta: cada cambio sube la versión y lo que vuelve de una
//! versión vieja se descarta. Si cambian varias teclas mientras una corre,
//! al terminar se busca solo la última.

use std::sync::Arc;

use nota_search::{FuzzyMatcher, Query, Ranked};

use super::input_line::{InputLine, LineEdit};

/// Cómo se eligió una fila.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Open {
    /// `enter` o clic.
    Here,
    /// `ctrl-v`: en un panel nuevo a la derecha.
    SplitRight,
    /// `ctrl-s`: en un panel nuevo abajo.
    SplitDown,
}

/// Una búsqueda por correr: es `Send` para irse a otro hilo.
#[derive(Debug)]
pub struct Job<T> {
    version: u64,
    query: Query,
    items: Arc<[T]>,
}

impl<T: AsRef<str>> Job<T> {
    #[must_use]
    pub fn run(self, matcher: &mut FuzzyMatcher) -> Matches {
        Matches {
            ranked: matcher.rank(&self.query, &self.items),
            version: self.version,
            query: self.query,
        }
    }
}

/// Lo que coincide con un query, del mejor al peor.
#[derive(Debug)]
pub struct Matches {
    version: u64,
    query: Query,
    ranked: Vec<Ranked>,
}

/// La fila elegida, por su posición en los ítems del picker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chosen {
    pub item: usize,
    pub open: Open,
}

/// Lo que hay que hacer tras un cambio del picker.
#[derive(Debug)]
#[must_use]
pub struct Step<T> {
    /// La búsqueda que falta correr.
    pub job: Option<Job<T>>,
    /// Se pidió elegir antes de que llegaran los ítems, y ya llegaron.
    pub chosen: Option<Chosen>,
}

#[derive(Debug)]
pub struct PickerState<T> {
    items: Arc<[T]>,
    line: InputLine,
    /// Sube con cada cambio de lo escrito o de los ítems.
    version: u64,
    shown: Matches,
    selected: usize,
    /// Hay una búsqueda corriendo.
    running: bool,
    /// Se pidió elegir antes de que llegaran los ítems.
    pending_choice: Option<Open>,
    /// Los próximos resultados vuelven a la primera fila.
    reset_selection: bool,
    /// Ya llegaron los ítems; antes, nada de lo mostrado es de verdad.
    loaded: bool,
}

impl<T: AsRef<str>> PickerState<T> {
    /// Con el query vacío, que muestra todos los ítems en su orden.
    #[must_use]
    pub fn new(items: Arc<[T]>) -> Self {
        let shown = everything(0, items.len());
        Self {
            items,
            line: InputLine::default(),
            version: 0,
            shown,
            selected: 0,
            running: false,
            pending_choice: None,
            reset_selection: false,
            loaded: true,
        }
    }

    /// Sin ítems todavía, como el finder mientras se indexa el vault: lo
    /// que se escriba y se elija espera a [`PickerState::set_items`].
    #[must_use]
    pub fn loading() -> Self {
        Self {
            loaded: false,
            ..Self::new(Arc::from([]))
        }
    }

    #[must_use]
    pub fn line(&self) -> &InputLine {
        &self.line
    }

    #[must_use]
    pub fn items(&self) -> &Arc<[T]> {
        &self.items
    }

    /// Cuántas filas tiene la lista.
    #[must_use]
    pub fn len(&self) -> usize {
        self.shown.ranked.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.shown.ranked.is_empty()
    }

    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// El ítem de la fila `row`.
    #[must_use]
    pub fn item(&self, row: usize) -> Option<&T> {
        let ranked = self.shown.ranked.get(row)?;
        self.items.get(ranked.index)
    }

    /// Qué bytes del ítem de `row` coinciden con el query de lo mostrado.
    /// Se calcula al pintar, solo para las filas visibles.
    pub fn positions(&self, row: usize, matcher: &mut FuzzyMatcher) -> Vec<usize> {
        match self.item(row) {
            Some(item) if !self.shown.query.is_empty() => {
                matcher.positions(&self.shown.query, item.as_ref())
            }
            _ => Vec::new(),
        }
    }

    /// Lo mostrado corresponde a lo escrito y a los ítems de ahora.
    #[must_use]
    pub fn is_current(&self) -> bool {
        self.loaded && self.shown.version == self.version
    }

    /// Edita lo escrito; si cambió el query, pide la búsqueda a correr.
    pub fn edit(&mut self, edit: LineEdit<'_>) -> Step<T> {
        if !self.line.apply(edit) {
            return Step {
                job: None,
                chosen: None,
            };
        }
        self.reset_selection = true;
        let job = self.changed();
        self.settle(job)
    }

    /// Cambia los ítems, como cuando el watcher trae otro índice del vault.
    /// La fila elegida se queda donde está, si sigue existiendo.
    pub fn set_items(&mut self, items: Arc<[T]>) -> Step<T> {
        self.items = items;
        self.loaded = true;
        let job = self.changed();
        self.settle(job)
    }

    fn changed(&mut self) -> Option<Job<T>> {
        self.version += 1;
        self.request()
    }

    /// La búsqueda de lo escrito ahora, salvo que ya corra una: al volver,
    /// [`PickerState::finish`] pide esta.
    fn request(&mut self) -> Option<Job<T>> {
        if !self.loaded {
            return None;
        }
        let query = Query::new(self.line.text());
        if query.is_empty() {
            // Sin query no hay nada que filtrar: se muestra todo ya.
            let shown = everything(self.version, self.items.len());
            self.show(shown);
            return None;
        }
        if self.running {
            return None;
        }
        self.running = true;
        Some(Job {
            version: self.version,
            query,
            items: Arc::clone(&self.items),
        })
    }

    /// Volvió una búsqueda: si es de lo escrito ahora, se muestra; si no,
    /// se pide la de ahora.
    pub fn finish(&mut self, matches: Matches) -> Step<T> {
        self.running = false;
        let job = if matches.version == self.version {
            self.show(matches);
            None
        } else {
            self.request()
        };
        self.settle(job)
    }

    /// Si había una elección esperando a los ítems y lo mostrado ya es de
    /// lo escrito, se elige.
    fn settle(&mut self, job: Option<Job<T>>) -> Step<T> {
        let chosen = if self.is_current() {
            self.pending_choice
                .take()
                .and_then(|open| self.chosen(open))
        } else {
            None
        };
        Step { job, chosen }
    }

    fn show(&mut self, matches: Matches) {
        self.shown = matches;
        if std::mem::take(&mut self.reset_selection) {
            self.selected = 0;
        }
        self.selected = self.selected.min(self.len().saturating_sub(1));
    }

    pub fn select_next(&mut self) {
        if !self.is_empty() {
            self.selected = (self.selected + 1) % self.len();
        }
    }

    pub fn select_previous(&mut self) {
        if !self.is_empty() {
            self.selected = self.selected.checked_sub(1).unwrap_or(self.len() - 1);
        }
    }

    /// Elige la fila `row`, como un clic.
    pub fn select(&mut self, row: usize) {
        if row < self.len() {
            self.selected = row;
        }
    }

    /// Elige la fila marcada de lo escrito ahora. Si la búsqueda de lo
    /// escrito no volvió todavía (se tecleó y se dio `enter` enseguida), se
    /// busca en el momento con `matcher`: es una sola búsqueda, la que se
    /// está esperando, y así las teclas que lleguen después del `enter` no
    /// cambian lo elegido. Sin ítems todavía, la elección espera a
    /// [`PickerState::set_items`].
    pub fn choose(&mut self, open: Open, matcher: &mut FuzzyMatcher) -> Option<Chosen> {
        if !self.loaded {
            self.pending_choice = Some(open);
            return None;
        }
        if !self.is_current() {
            let job = Job {
                version: self.version,
                query: Query::new(self.line.text()),
                items: Arc::clone(&self.items),
            };
            let found = job.run(matcher);
            self.show(found);
        }
        self.chosen(open)
    }

    fn chosen(&self, open: Open) -> Option<Chosen> {
        let ranked = self.shown.ranked.get(self.selected)?;
        Some(Chosen {
            item: ranked.index,
            open,
        })
    }
}

fn everything(version: u64, len: usize) -> Matches {
    Matches {
        version,
        query: Query::new(""),
        ranked: (0..len).map(|index| Ranked { index, score: 0 }).collect(),
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
