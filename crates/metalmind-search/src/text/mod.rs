//! Búsqueda de texto completo en las notas del vault: un grep paralelo con
//! las librerías de ripgrep, cancelable y con los resultados en streaming.

mod note;
mod query;

use std::collections::HashSet;
use std::io;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};

use metalmind_vault::VaultPath;

use note::NoteSearcher;
pub use note::{LineMatch, TextMatcher};
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
    #[error("falló el motor de búsqueda: {0}")]
    Engine(#[source] io::Error),
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
        let matcher = query.compile()?;
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
            .name("metalmind-search texto".to_owned())
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
    matcher: TextMatcher,
    scope: Option<Arc<HashSet<VaultPath>>>,
    cancel: Arc<AtomicBool>,
}

impl Job {
    fn run(&self, sender: &mpsc::Sender<NoteMatches>) {
        metalmind_vault::walk_notes(&self.root, || {
            let mut note = NoteSearcher::new(self.matcher.clone());
            let sender = sender.clone();
            move |path, file: &Path| self.visit(path, file, &mut note, &sender)
        });
    }

    fn visit(
        &self,
        path: VaultPath,
        file: &Path,
        note: &mut NoteSearcher,
        sender: &mpsc::Sender<NoteMatches>,
    ) -> ControlFlow<()> {
        if self.cancel.load(Ordering::Relaxed) {
            return ControlFlow::Break(());
        }
        if self
            .scope
            .as_ref()
            .is_some_and(|scope| !scope.contains(&path))
        {
            return ControlFlow::Continue(());
        }
        let lines = match note.search(file, &self.cancel) {
            Ok(lines) => lines,
            Err(error) => {
                log::warn!("al buscar en {path}: {error}");
                return ControlFlow::Continue(());
            }
        };
        if lines.is_empty() {
            return ControlFlow::Continue(());
        }
        if sender.send(NoteMatches { path, lines }).is_err() {
            // Nadie escucha: es lo mismo que cancelar.
            self.cancel.store(true, Ordering::Relaxed);
            return ControlFlow::Break(());
        }
        ControlFlow::Continue(())
    }
}
