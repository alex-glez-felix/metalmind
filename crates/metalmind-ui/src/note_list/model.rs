//! La lista sin GPUI: las notas de su fuente de la más reciente a la más
//! vieja, filtradas en su lugar con la consulta del finder (`#rust
//! -#archivado parser`, 05 §4) y la fila elegida, que sigue a su nota
//! aunque la lista cambie.

use std::sync::Arc;

use metalmind_vault::tags::Tags;
use metalmind_vault::{EntryKind, VaultChange, VaultIndex, VaultPath, VaultUpdate};

use super::cards::{Card, Cards};
use super::source::ListSource;
use crate::finder::{Note, TagFilter};
use crate::picker::{InputLine, LineEdit, Matches, PickerState, Step};

/// Lo que la lista le pregunta al usuario debajo de su cabecera.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prompt {
    /// Escribiendo el filtro, que vive en el estado del picker.
    Filter,
    /// Los tags para la nota elegida (`t`).
    Tag,
}

pub struct NoteList {
    source: ListSource,
    index: Arc<VaultIndex>,
    tags: Arc<Tags>,
    cards: Cards,
    /// Ya llegaron las primeras tarjetas: antes no se muestra nada.
    cards_ready: bool,
    /// Las notas de la fuente en orden y lo que deja el filtro.
    filter: PickerState<Note>,
    /// La nota de la fila elegida: la fila la sigue cuando la lista cambia.
    selected: Option<VaultPath>,
    prompt: Option<Prompt>,
    tag_line: InputLine,
}

impl NoteList {
    /// Sin tarjetas todavía: no muestra nada hasta [`NoteList::set_cards`],
    /// porque sin fechas el orden cambiaría al llegar.
    #[must_use]
    pub fn new(source: ListSource, filter: &str, index: Arc<VaultIndex>, tags: Arc<Tags>) -> Self {
        let mut state =
            PickerState::loading().with_narrowing(Arc::new(TagFilter::new(Arc::clone(&tags))));
        // Sin ítems, editar no pide ninguna búsqueda.
        let _ = state.edit(LineEdit::Insert(filter));
        Self {
            source,
            index,
            tags,
            cards: Cards::default(),
            cards_ready: false,
            filter: state,
            selected: None,
            prompt: None,
            tag_line: InputLine::default(),
        }
    }

    #[must_use]
    pub fn source(&self) -> &ListSource {
        &self.source
    }

    #[must_use]
    pub fn tags(&self) -> &Tags {
        &self.tags
    }

    /// Lo escrito en el filtro.
    #[must_use]
    pub fn filter(&self) -> &InputLine {
        self.filter.line()
    }

    #[must_use]
    pub fn warnings(&self) -> Vec<std::ops::Range<usize>> {
        self.filter.warnings()
    }

    #[must_use]
    pub fn prompt(&self) -> Option<Prompt> {
        self.prompt
    }

    #[must_use]
    pub fn tag_line(&self) -> &InputLine {
        &self.tag_line
    }

    /// Ya llegaron las tarjetas y lo mostrado es de lo escrito.
    #[must_use]
    pub fn is_current(&self) -> bool {
        self.filter.is_current()
    }

    /// Cuántas notas trae la fuente.
    #[must_use]
    pub fn total(&self) -> usize {
        self.filter.items().len()
    }

    /// Cuántas filas deja el filtro.
    #[must_use]
    pub fn len(&self) -> usize {
        self.filter.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.filter.is_empty()
    }

    #[must_use]
    pub fn selected_row(&self) -> Option<usize> {
        (!self.is_empty()).then(|| self.filter.selected())
    }

    #[must_use]
    pub fn selected_note(&self) -> Option<&VaultPath> {
        self.note(self.selected_row()?)
    }

    #[must_use]
    pub fn note(&self, row: usize) -> Option<&VaultPath> {
        self.filter.item(row).map(|note| &note.0)
    }

    #[must_use]
    pub fn card(&self, row: usize) -> Option<&Card> {
        self.cards.get(self.note(row)?)
    }

    /// Llegaron tarjetas leídas del disco. Las de una nota que ya no está
    /// (se leyó antes de un renombre) no se guardan.
    pub fn set_cards(&mut self, cards: Vec<(VaultPath, Card)>) -> Step<Note> {
        let index = Arc::clone(&self.index);
        self.cards.insert_all(
            cards
                .into_iter()
                .filter(|(path, _)| index.kind(path) == Some(EntryKind::Note)),
        );
        self.cards_ready = true;
        self.refill()
    }

    /// Un lote del watcher: devuelve lo que muestra y las notas cuya tarjeta
    /// hay que releer.
    pub fn apply(&mut self, update: &VaultUpdate) -> (Step<Note>, Vec<VaultPath>) {
        self.index = Arc::clone(&update.snapshot);
        self.tags = Arc::clone(&update.tags);
        self.source.follow(&update.changes);
        self.selected = self
            .selected
            .take()
            .map(|selected| renamed(selected, &update.changes));
        let stale = self.cards.apply(&update.changes, &self.index);
        if !self.cards_ready {
            // Sin fechas no hay orden: se muestra al llegar las tarjetas.
            return (Step::default(), stale);
        }
        (self.refill(), stale)
    }

    /// Un tag que se renombró o fusionó desde la app sigue siendo la fuente
    /// con su nombre nuevo.
    pub fn rename_tag(&mut self, from: &str, to: &str) -> Step<Note> {
        if !self.source.rename_tag(from, to) {
            return Step::default();
        }
        self.refill()
    }

    /// Otra fuente: el filtro se queda y la fila vuelve a la primera.
    pub fn set_source(&mut self, source: ListSource) -> Step<Note> {
        self.source = source;
        self.selected = None;
        self.filter.select(0);
        self.refill()
    }

    fn refill(&mut self) -> Step<Note> {
        let narrowing = Arc::new(TagFilter::new(Arc::clone(&self.tags)));
        let step = self.filter.set_source(self.ordered(), narrowing);
        self.follow(step)
    }

    /// Las notas de la fuente, de la que cambió último a la primera; las
    /// que tienen la misma fecha, por ruta.
    fn ordered(&self) -> Arc<[Note]> {
        let mut notes = self.source.notes(&self.index, &self.tags);
        notes.sort_by_cached_key(|path| {
            let modified = self.cards.get(path).and_then(|card| card.modified);
            (std::cmp::Reverse(modified), path.clone())
        });
        notes.into_iter().map(Note).collect()
    }

    pub fn edit_filter(&mut self, edit: LineEdit<'_>) -> Step<Note> {
        self.selected = None;
        let step = self.filter.edit(edit);
        self.follow(step)
    }

    /// Volvió una búsqueda del filtro.
    pub fn finish(&mut self, matches: Matches) -> Step<Note> {
        let step = self.filter.finish(matches);
        self.follow(step)
    }

    /// Si lo mostrado ya es de lo escrito, la fila vuelve a la nota que
    /// estaba elegida, o toma la nota de la fila si esa ya no está.
    fn follow(&mut self, step: Step<Note>) -> Step<Note> {
        if self.filter.is_current() {
            let row = self
                .selected
                .as_ref()
                .and_then(|selected| (0..self.len()).find(|&row| self.note(row) == Some(selected)));
            match row {
                Some(row) => self.filter.select(row),
                None => self.selected = self.selected_note().cloned(),
            }
        }
        step
    }

    pub fn select(&mut self, row: usize) {
        self.filter.select(row);
        self.selected = self.selected_note().cloned();
    }

    /// Baja `count` filas, sin pasar de la última.
    pub fn select_next(&mut self, count: usize) {
        if let Some(row) = self.selected_row() {
            self.select(row.saturating_add(count).min(self.len() - 1));
        }
    }

    pub fn select_previous(&mut self, count: usize) {
        if let Some(row) = self.selected_row() {
            self.select(row.saturating_sub(count));
        }
    }

    pub fn select_first(&mut self) {
        self.select(0);
    }

    pub fn select_last(&mut self) {
        self.select(self.len().saturating_sub(1));
    }

    /// Elige la fila de `path`, si la lista la muestra.
    pub fn reveal(&mut self, path: &VaultPath) -> Option<usize> {
        let row = (0..self.len()).find(|&row| self.note(row) == Some(path))?;
        self.select(row);
        Some(row)
    }

    pub fn start_filter(&mut self) {
        self.prompt = Some(Prompt::Filter);
    }

    /// Pregunta los tags de la nota elegida, si hay una.
    pub fn start_tagging(&mut self) -> bool {
        if self.selected_note().is_none() {
            return false;
        }
        self.tag_line = InputLine::default();
        self.prompt = Some(Prompt::Tag);
        true
    }

    pub fn edit_tag_line(&mut self, edit: LineEdit<'_>) {
        self.tag_line.apply(edit);
    }

    /// Cierra la pregunta. Del filtro, lo escrito se queda.
    pub fn close_prompt(&mut self) {
        self.prompt = None;
    }

    /// Lo escrito para la nota elegida, como nombres de tag; cierra la
    /// pregunta.
    pub fn take_tags(&mut self) -> Option<(VaultPath, Vec<String>)> {
        self.prompt = None;
        let note = self.selected_note()?.clone();
        let tags: Vec<String> = self
            .tag_line
            .text()
            .split([' ', ','])
            .filter(|word| !word.is_empty())
            .map(str::to_owned)
            .collect();
        (!tags.is_empty()).then_some((note, tags))
    }
}

/// Dónde quedó `path` tras los renombres del lote.
fn renamed(path: VaultPath, changes: &[VaultChange]) -> VaultPath {
    changes.iter().fold(path, |path, change| match change {
        VaultChange::Renamed { from, to, .. } => path.rebase(from, to).unwrap_or(path),
        _ => path,
    })
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
