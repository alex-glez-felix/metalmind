//! El estado de `space /` sin GPUI: lo escrito, las líneas que coinciden y
//! la fila elegida. Cada patrón válido es una búsqueda con su versión, que
//! corre en el hilo de [`SearchWorker`](super::worker::SearchWorker); las
//! notas que llegan de otra versión se descartan.
//!
//! Los resultados de antes se ven hasta que llega el primero de la búsqueda
//! nueva, así la lista no parpadea vacía en cada tecla. Un patrón inválido
//! no cambia nada: la búsqueda anterior sigue y el error se muestra.

use std::collections::HashSet;
use std::sync::Arc;

use metalmind_search::{NoteMatches, Syntax, TextQuery};
use metalmind_vault::VaultPath;

use crate::picker::{Chosen, InputLine, LineEdit, Open};

use super::Hit;

/// Con más líneas que esto la búsqueda se corta: hay que afinarla.
pub const MAX_HITS: usize = 10_000;

/// Una búsqueda por lanzar.
#[derive(Debug)]
pub struct Search {
    pub version: u64,
    pub query: TextQuery,
}

/// En qué va la búsqueda de lo que se muestra.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// No hay nada escrito.
    Idle,
    Searching,
    Done,
    /// Había más de [`MAX_HITS`] líneas y se cortó.
    Truncated,
    /// Lo escrito no compila; lo mostrado es de la búsqueda anterior.
    Invalid(String),
    /// No se pudo buscar.
    Failed(String),
}

#[derive(Debug)]
pub struct TextSearchState {
    line: InputLine,
    /// La versión de la búsqueda cuyos resultados se aceptan.
    version: u64,
    hits: Vec<Hit>,
    /// Lo mostrado es de una búsqueda anterior a `version`.
    stale: bool,
    selected: usize,
    /// En qué va la búsqueda de `version`.
    progress: Status,
    /// Por qué no compila lo escrito, si no compila.
    invalid: Option<String>,
    /// Se pidió elegir antes de que llegaran resultados.
    pending_choice: Option<Open>,
    /// Con focus, las notas donde se busca.
    scope: Option<Arc<HashSet<VaultPath>>>,
}

impl Default for TextSearchState {
    fn default() -> Self {
        Self::new()
    }
}

impl TextSearchState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            line: InputLine::default(),
            version: 0,
            hits: Vec::new(),
            stale: false,
            selected: 0,
            progress: Status::Idle,
            invalid: None,
            pending_choice: None,
            scope: None,
        }
    }

    /// Busca solo en `scope`: las notas del focus.
    #[must_use]
    pub fn with_scope(mut self, scope: Arc<HashSet<VaultPath>>) -> Self {
        self.scope = Some(scope);
        self
    }

    #[must_use]
    pub fn line(&self) -> &InputLine {
        &self.line
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.hits.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.hits.is_empty()
    }

    #[must_use]
    pub fn hit(&self, row: usize) -> Option<&Hit> {
        self.hits.get(row)
    }

    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Lo mostrado es de una búsqueda anterior a lo escrito: todavía no
    /// llegó nada de la actual.
    #[must_use]
    pub fn is_stale(&self) -> bool {
        self.stale
    }

    #[must_use]
    pub fn status(&self) -> Status {
        match &self.invalid {
            Some(error) => Status::Invalid(error.clone()),
            None => self.progress.clone(),
        }
    }

    /// Edita lo escrito. Si queda un patrón válido distinto, devuelve la
    /// búsqueda a lanzar; un patrón vacío también, para cancelar la anterior.
    pub fn edit(&mut self, edit: LineEdit<'_>) -> Option<Search> {
        if self.pending_choice.is_some() || !self.line.apply(edit) {
            return None;
        }
        let query = TextQuery {
            pattern: self.line.text().to_owned(),
            syntax: Syntax::Regex,
            scope: self.scope.clone(),
            ..TextQuery::default()
        };
        if let Err(error) = query.compile() {
            self.invalid = Some(error.to_string());
            return None;
        }
        self.invalid = None;
        self.version += 1;
        self.stale = true;
        if query.pattern.is_empty() {
            self.show_nothing(Status::Idle);
        } else {
            self.progress = Status::Searching;
        }
        Some(Search {
            version: self.version,
            query,
        })
    }

    /// Llegaron notas de la búsqueda `version`.
    /// Si se había pedido elegir antes de que llegaran resultados, devuelve
    /// lo elegido.
    pub fn receive(&mut self, version: u64, notes: Vec<NoteMatches>) -> Option<Chosen> {
        if version != self.version || self.progress != Status::Searching || notes.is_empty() {
            return None;
        }
        self.add(notes)
    }

    fn add(&mut self, notes: Vec<NoteMatches>) -> Option<Chosen> {
        if std::mem::take(&mut self.stale) {
            self.hits.clear();
            self.selected = 0;
        }
        for note in notes {
            let room = MAX_HITS - self.hits.len();
            if note.lines.len() > room {
                self.progress = Status::Truncated;
            }
            let path = note.path;
            self.hits
                .extend(note.lines.into_iter().take(room).map(|line| Hit {
                    path: path.clone(),
                    line,
                }));
            if self.progress == Status::Truncated {
                break;
            }
        }
        let open = self.pending_choice.take()?;
        self.chosen(open)
    }

    /// Terminó la búsqueda `version`: si no trajo nada, no hay resultados.
    pub fn finished(&mut self, version: u64) {
        if version != self.version {
            return;
        }
        if self.stale {
            self.show_nothing(Status::Done);
        } else if self.progress == Status::Searching {
            self.progress = Status::Done;
        }
    }

    /// La búsqueda `version` no pudo seguir: lo que ya llegó de ella se
    /// queda, con el error.
    pub fn failed(&mut self, version: u64, error: String) {
        if version != self.version {
            return;
        }
        if self.stale {
            self.show_nothing(Status::Failed(error));
        } else {
            self.pending_choice = None;
            self.progress = Status::Failed(error);
        }
    }

    fn show_nothing(&mut self, status: Status) {
        self.hits.clear();
        self.selected = 0;
        self.stale = false;
        self.pending_choice = None;
        self.progress = status;
    }

    pub fn select(&mut self, row: usize) {
        if row < self.len() {
            self.selected = row;
        }
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

    /// Elige la fila marcada. Si lo mostrado es de una búsqueda anterior
    /// (se tecleó y se dio `enter` enseguida), la elección espera al primer
    /// resultado de lo escrito, y lo que se teclee mientras no cambia nada;
    /// con lo escrito inválido no espera: elegiría algo que no se vio.
    pub fn choose(&mut self, open: Open) -> Option<Chosen> {
        if self.stale && self.progress == Status::Searching {
            if self.invalid.is_none() {
                self.pending_choice = Some(open);
            }
            return None;
        }
        self.chosen(open)
    }

    fn chosen(&self, open: Open) -> Option<Chosen> {
        (self.selected < self.len()).then_some(Chosen {
            item: self.selected,
            open,
        })
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
