//! Búsqueda de texto completo en las notas del vault: un grep paralelo con
//! las librerías de ripgrep, cancelable y con los resultados en streaming.

mod note;
mod query;

use std::collections::HashSet;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};

use grep_regex::RegexMatcher;
use ignore::{DirEntry, WalkState};
use nota_vault::{EntryKind, VaultPath};

pub use note::LineMatch;
use note::NoteSearcher;
pub use query::{Case, Syntax, TextQuery};

/// Las coincidencias de una nota, todas juntas: llegan nota por nota.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteMatches {
    pub path: VaultPath,
    /// En el orden de la nota.
    pub lines: Vec<LineMatch>,
}

#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error("patrón inválido: {0}")]
    InvalidPattern(#[from] grep_regex::Error),
    #[error("no se pudo lanzar el hilo de búsqueda: {0}")]
    Spawn(#[source] io::Error),
}

/// Busca texto en las notas de un vault, una búsqueda a la vez: empezar otra
/// cancela la que estaba corriendo.
pub struct TextSearcher {
    root: Arc<PathBuf>,
    running: Option<Running>,
}

struct Running {
    cancel: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

impl TextSearcher {
    /// `root` es la raíz del vault, la de `Vault::root`.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: Arc::new(root.into()),
            running: None,
        }
    }

    /// Cancela la búsqueda anterior y empieza `query` en paralelo, con las
    /// mismas reglas que el índice del vault (`.gitignore`, sin ocultos ni
    /// enlaces simbólicos). Las notas que coinciden llegan por el receptor en
    /// cualquier orden; se desconecta al terminar o al cancelar. Soltarlo la
    /// corta en la próxima nota que coincida.
    ///
    /// # Errors
    /// Si el patrón no compila; entonces la búsqueda anterior sigue.
    pub fn search(
        &mut self,
        query: &TextQuery,
    ) -> Result<mpsc::Receiver<NoteMatches>, SearchError> {
        let matcher = query.matcher()?;
        self.cancel();
        let (sender, receiver) = mpsc::channel();
        if query.pattern.is_empty() {
            return Ok(receiver);
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let job = Job {
            root: Arc::clone(&self.root),
            matcher,
            scope: query.scope.clone(),
            cancel: Arc::clone(&cancel),
        };
        let thread = thread::Builder::new()
            .name("nota-search texto".to_owned())
            .spawn(move || job.run(&sender))
            .map_err(SearchError::Spawn)?;
        self.running = Some(Running { cancel, thread });
        Ok(receiver)
    }

    /// Para la búsqueda que esté corriendo y espera a que sus hilos terminen:
    /// después no llega nada más por su receptor.
    pub fn cancel(&mut self) {
        if let Some(running) = self.running.take() {
            running.cancel.store(true, Ordering::Relaxed);
            if running.thread.join().is_err() {
                log::error!("la búsqueda de texto terminó en pánico");
            }
        }
    }
}

impl Drop for TextSearcher {
    fn drop(&mut self) {
        self.cancel();
    }
}

struct Job {
    root: Arc<PathBuf>,
    matcher: RegexMatcher,
    scope: Option<Arc<HashSet<VaultPath>>>,
    cancel: Arc<AtomicBool>,
}

impl Job {
    fn run(&self, sender: &mpsc::Sender<NoteMatches>) {
        nota_vault::walker(&self.root).build_parallel().run(|| {
            let mut note = NoteSearcher::new(self.matcher.clone());
            let sender = sender.clone();
            Box::new(move |entry| self.visit(entry, &mut note, &sender))
        });
    }

    fn visit(
        &self,
        entry: Result<DirEntry, ignore::Error>,
        note: &mut NoteSearcher,
        sender: &mpsc::Sender<NoteMatches>,
    ) -> WalkState {
        if self.cancel.load(Ordering::Relaxed) {
            return WalkState::Quit;
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                log::warn!("al buscar en el vault: {error}");
                return WalkState::Continue;
            }
        };
        let Some((path, EntryKind::Note)) = nota_vault::classify(&self.root, &entry) else {
            return WalkState::Continue;
        };
        if self
            .scope
            .as_ref()
            .is_some_and(|scope| !scope.contains(&path))
        {
            return WalkState::Continue;
        }
        let lines = match note.search(entry.path(), &self.cancel) {
            Ok(lines) => lines,
            Err(error) => {
                log::warn!("al buscar en {path}: {error}");
                return WalkState::Continue;
            }
        };
        if lines.is_empty() {
            return WalkState::Continue;
        }
        if sender.send(NoteMatches { path, lines }).is_err() {
            // Nadie escucha: es lo mismo que cancelar.
            self.cancel.store(true, Ordering::Relaxed);
            return WalkState::Quit;
        }
        WalkState::Continue
    }
}
