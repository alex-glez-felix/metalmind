use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use arc_swap::ArcSwap;
use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{DebounceEventResult, Debouncer, RecommendedCache, new_debouncer};

use crate::file;
use crate::index::VaultIndex;
use crate::links::LinkIndex;
use crate::links::RenamePlan;
use crate::ops::{self, FileOpError};
use crate::path::VaultPath;
use crate::refactor::{self, AppliedRename, ApplyRenameError, UndoReport};
use crate::refresh::{Touched, VaultChange, refresh};
use crate::scan::scan;
use crate::watch::{touched, touched_by_errors};

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

/// Lo que cambió en el vault, con los índices que ya lo incluyen.
#[derive(Debug, Clone)]
pub struct VaultUpdate {
    pub changes: Vec<VaultChange>,
    pub snapshot: Arc<VaultIndex>,
    pub links: Arc<LinkIndex>,
}

/// Un vault abierto: la carpeta raíz, su índice y el watcher que lo mantiene
/// al día en un hilo propio.
pub struct Vault {
    root: PathBuf,
    index: Arc<ArcSwap<VaultIndex>>,
    links: Arc<ArcSwap<LinkIndex>>,
    watch: Option<Watch>,
}

struct Watch {
    debouncer: Debouncer<RecommendedWatcher, RecommendedCache>,
    worker: JoinHandle<()>,
    stop: Arc<AtomicBool>,
}

impl Vault {
    /// Abre la carpeta `root`, la indexa y empieza a vigilarla. Cada cambio en
    /// disco llega por el receptor como un [`VaultUpdate`]. Quien no quiera
    /// los cambios suelta el receptor y usa [`Vault::snapshot`], que sigue al
    /// día igual; guardarlo sin leerlo acumula un snapshot por lote.
    ///
    /// # Errors
    /// Si `root` no existe, no es una carpeta o no se puede vigilar.
    pub fn open(root: impl AsRef<Path>) -> Result<(Self, mpsc::Receiver<VaultUpdate>), VaultError> {
        let root = canonical_dir(root.as_ref())?;
        let (events, raw) = mpsc::channel();
        let mut debouncer = new_debouncer(DEBOUNCE, None, events)?;
        // Vigilar antes del walk: lo que cambie mientras tanto llega como evento.
        debouncer.watch(&root, RecursiveMode::Recursive)?;
        let files = scan(&root);
        let links = LinkIndex::build(
            files.files().map(|entry| (entry.path.clone(), entry.kind)),
            |path| read_note(&root, path),
        );
        let index = Arc::new(ArcSwap::from_pointee(files));
        let links = Arc::new(ArcSwap::from_pointee(links));
        let (updates, receiver) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let worker = Worker {
            root: root.clone(),
            index: Arc::clone(&index),
            links: Arc::clone(&links),
            updates,
            stop: Arc::clone(&stop),
        };
        let worker = thread::Builder::new()
            .name("nota-vault watcher".to_owned())
            .spawn(move || worker.run(&raw))
            .map_err(VaultError::Spawn)?;
        let vault = Self {
            root,
            index,
            links,
            watch: Some(Watch {
                debouncer,
                worker,
                stop,
            }),
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

    /// Los links del vault en este momento, al día con [`Vault::snapshot`]
    /// salvo entre la publicación de uno y otro.
    #[must_use]
    pub fn links(&self) -> Arc<LinkIndex> {
        self.links.load_full()
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

    /// Aplica un plan de [`LinkIndex::rename_plan`]: comprueba que cada
    /// archivo dice lo que el plan espera, reescribe sus links y renombra. Lo
    /// que quien llama aplique por su cuenta, como los buffers abiertos con
    /// cambios sin guardar, se quita de `plan.files` antes. El índice se
    /// entera por el watcher.
    ///
    /// # Errors
    /// Si algún archivo cambió, no se puede leer o el destino existe, sin
    /// escribir nada; si falla a medias, [`ApplyRenameError::Interrupted`]
    /// con lo que quedó hecho, para deshacerlo.
    pub fn apply_rename(&self, plan: &RenamePlan) -> Result<AppliedRename, ApplyRenameError> {
        refactor::apply(&self.root, plan)
    }

    /// Deshace un plan aplicado: devuelve cada archivo a su texto de antes,
    /// salvo los que cambiaron desde entonces, y renombra de vuelta.
    ///
    /// # Errors
    /// Si la ruta de antes del rename ya está ocupada; entonces no toca nada.
    pub fn undo_rename(&self, applied: &AppliedRename) -> Result<UndoReport, FileOpError> {
        refactor::undo(&self.root, applied)
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
        let Some(Watch {
            debouncer,
            worker,
            stop,
        }) = self.watch.take()
        else {
            return;
        };
        // El hilo deja de releer lo pendiente; al parar, el debouncer suelta su
        // emisor y el hilo sale del bucle.
        stop.store(true, Ordering::Release);
        debouncer.stop();
        if worker.join().is_err() {
            log::error!("el watcher de {} terminó en pánico", self.root.display());
        }
    }
}

struct Worker {
    root: PathBuf,
    index: Arc<ArcSwap<VaultIndex>>,
    links: Arc<ArcSwap<LinkIndex>>,
    updates: mpsc::Sender<VaultUpdate>,
    stop: Arc<AtomicBool>,
}

impl Worker {
    fn run(self, events: &mpsc::Receiver<DebounceEventResult>) {
        let mut current = VaultIndex::clone(&self.index.load());
        let mut links = LinkIndex::clone(&self.links.load());
        for first in events {
            // Si el refresh se atrasó, todo lo que espera se relee en una sola pasada.
            let mut touched = self.touched(first);
            for more in events.try_iter() {
                touched.merge(self.touched(more));
            }
            if self.stop.load(Ordering::Acquire) {
                return;
            }
            let changes = refresh(&mut current, &self.root, &touched);
            if changes.is_empty() {
                continue;
            }
            for change in &changes {
                links.apply(change, |path| read_note(&self.root, path));
            }
            let snapshot = Arc::new(current.clone());
            let published = Arc::new(links.clone());
            self.index.store(Arc::clone(&snapshot));
            self.links.store(Arc::clone(&published));
            // Si el receptor ya no existe, quien abrió el vault solo usa los snapshots.
            let _ = self.updates.send(VaultUpdate {
                changes,
                snapshot,
                links: published,
            });
        }
    }

    fn touched(&self, result: DebounceEventResult) -> Touched {
        match result {
            Ok(events) => touched(&self.root, &events),
            Err(errors) => {
                for error in &errors {
                    log::warn!("watcher de {}: {error}", self.root.display());
                }
                touched_by_errors(&self.root, &errors)
            }
        }
    }
}

/// El contenido de una nota para el índice de links; si no se puede leer,
/// queda sin links hasta su próximo cambio.
fn read_note(root: &Path, path: &VaultPath) -> Option<String> {
    match file::read(&path.to_fs(root)) {
        Ok(text) => text,
        Err(error) => {
            log::warn!("{error}");
            None
        }
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
