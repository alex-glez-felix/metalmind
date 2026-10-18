use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use arc_swap::ArcSwap;
use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{DebounceEventResult, Debouncer, RecommendedCache, new_debouncer};

use crate::index::VaultIndex;
use crate::ops::{self, FileOpError};
use crate::path::VaultPath;
use crate::refresh::{Touched, VaultChange, refresh};
use crate::scan::scan;
use crate::watch::touched;

/// Cuánto tiene que calmarse una ruta antes de releerla. Junta el temporal y
/// el rename de un guardado atómico, y una ráfaga en un solo lote.
const DEBOUNCE: Duration = Duration::from_millis(100);

#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("no se pudo abrir el vault {}: {source}", root.display())]
    Open { root: PathBuf, source: io::Error },
    #[error("{} no es una carpeta", .0.display())]
    NotADirectory(PathBuf),
    #[error("no se pudo vigilar el vault: {0}")]
    Watch(#[from] notify::Error),
    #[error("no se pudo arrancar el hilo del watcher: {0}")]
    Spawn(io::Error),
}

/// Lo que cambió en el vault, con el índice que ya lo incluye.
#[derive(Debug, Clone)]
pub struct VaultUpdate {
    pub changes: Vec<VaultChange>,
    pub snapshot: Arc<VaultIndex>,
}

/// Un vault abierto: la carpeta raíz, su índice y el watcher que lo mantiene
/// al día en un hilo propio.
pub struct Vault {
    root: PathBuf,
    index: Arc<ArcSwap<VaultIndex>>,
    watch: Option<Watch>,
}

struct Watch {
    debouncer: Debouncer<RecommendedWatcher, RecommendedCache>,
    worker: JoinHandle<()>,
}

impl Vault {
    /// Abre la carpeta `root`, la indexa y empieza a vigilarla. Cada cambio en
    /// disco llega por el receptor como un [`VaultUpdate`]; si nadie lo lee,
    /// [`Vault::snapshot`] sigue al día igual.
    ///
    /// # Errors
    /// Si `root` no existe, no es una carpeta o no se puede vigilar.
    pub fn open(root: impl AsRef<Path>) -> Result<(Self, mpsc::Receiver<VaultUpdate>), VaultError> {
        let root = canonical_dir(root.as_ref())?;
        let (events, raw) = mpsc::channel();
        let mut debouncer = new_debouncer(DEBOUNCE, None, events)?;
        // Vigilar antes del walk: lo que cambie mientras tanto llega como evento.
        debouncer.watch(&root, RecursiveMode::Recursive)?;
        let index = Arc::new(ArcSwap::from_pointee(scan(&root)));
        let (updates, receiver) = mpsc::channel();
        let worker = Worker {
            root: root.clone(),
            index: Arc::clone(&index),
            updates,
        };
        let worker = thread::Builder::new()
            .name("nota-vault watcher".to_owned())
            .spawn(move || worker.run(&raw))
            .map_err(VaultError::Spawn)?;
        let vault = Self {
            root,
            index,
            watch: Some(Watch { debouncer, worker }),
        };
        Ok((vault, receiver))
    }

    /// La raíz, sin enlaces simbólicos.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// El índice de este momento. No bloquea ni se modifica después.
    #[must_use]
    pub fn snapshot(&self) -> Arc<VaultIndex> {
        self.index.load_full()
    }

    /// Crea una nota vacía, con las carpetas que le falten. El índice se
    /// entera por el watcher, como de cualquier otro cambio en disco.
    ///
    /// # Errors
    /// Si no termina en `.md`, ya existe o pasa por un enlace simbólico.
    pub fn create_note(&self, path: &VaultPath) -> Result<(), FileOpError> {
        ops::create_note(&self.root, path)
    }

    /// Crea una carpeta vacía, con las que le falten.
    ///
    /// # Errors
    /// Si ya existe o pasa por un enlace simbólico.
    pub fn create_folder(&self, path: &VaultPath) -> Result<(), FileOpError> {
        ops::create_folder(&self.root, path)
    }

    /// Renombra o mueve un archivo o carpeta, creando las carpetas que le
    /// falten al destino. Nunca pisa lo que ya exista ahí.
    ///
    /// # Errors
    /// Si `from` no existe o es la raíz, `to` ya existe o está dentro de
    /// `from`, o alguno pasa por un enlace simbólico.
    pub fn rename(&self, from: &VaultPath, to: &VaultPath) -> Result<(), FileOpError> {
        ops::rename(&self.root, from, to)
    }

    /// Manda un archivo o carpeta a la papelera del vault (`.trash/`, como
    /// Obsidian) y devuelve dónde quedó.
    ///
    /// # Errors
    /// Si no existe, es la raíz o pasa por un enlace simbólico.
    pub fn move_to_trash(&self, path: &VaultPath) -> Result<PathBuf, FileOpError> {
        ops::trash(&self.root, path)
    }
}

impl Drop for Vault {
    fn drop(&mut self) {
        let Some(Watch { debouncer, worker }) = self.watch.take() else {
            return;
        };
        // Al parar, el debouncer suelta su emisor y el hilo sale del bucle.
        debouncer.stop();
        if worker.join().is_err() {
            log::error!("el watcher de {} terminó en pánico", self.root.display());
        }
    }
}

struct Worker {
    root: PathBuf,
    index: Arc<ArcSwap<VaultIndex>>,
    updates: mpsc::Sender<VaultUpdate>,
}

impl Worker {
    fn run(self, events: &mpsc::Receiver<DebounceEventResult>) {
        let mut current = VaultIndex::clone(&self.index.load());
        for result in events {
            let touched = match result {
                Ok(events) => touched(&self.root, &events),
                Err(errors) => self.touched_by_errors(errors),
            };
            let changes = refresh(&mut current, &self.root, &touched);
            if changes.is_empty() {
                continue;
            }
            let snapshot = Arc::new(current.clone());
            self.index.store(Arc::clone(&snapshot));
            // Si el receptor ya no existe, quien abrió el vault solo usa `snapshot`.
            let _ = self.updates.send(VaultUpdate { changes, snapshot });
        }
    }

    /// Tras un error del watcher pudo perderse algo: se relee lo que el error
    /// nombra o, si no nombra nada, todo el vault.
    fn touched_by_errors(&self, errors: Vec<notify::Error>) -> Touched {
        let mut touched = Touched::default();
        for error in errors {
            log::warn!("watcher de {}: {error}", self.root.display());
            if error.paths.is_empty() {
                touched.paths.insert(VaultPath::root());
            }
            touched.paths.extend(
                error
                    .paths
                    .iter()
                    .filter_map(|path| VaultPath::from_fs(&self.root, path)),
            );
        }
        touched
    }
}

fn canonical_dir(root: &Path) -> Result<PathBuf, VaultError> {
    let canonical = root.canonicalize().map_err(|source| VaultError::Open {
        root: root.to_owned(),
        source,
    })?;
    if canonical.is_dir() {
        Ok(canonical)
    } else {
        Err(VaultError::NotADirectory(root.to_owned()))
    }
}
