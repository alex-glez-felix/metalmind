use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use arc_swap::ArcSwap;
use jiff::civil::DateTime;
use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{DebounceEventResult, Debouncer, RecommendedCache, new_debouncer};

use crate::apply::{self, Applied, ApplyError, UndoReport};
use crate::attachment::{self, AttachmentRefs, NewAttachment, SavedAttachment};
use crate::capture::{self, CaptureError, Captured, NewNote};
use crate::changes::Changes;
use crate::file::{self, FileError};
use crate::index::{EntryKind, VaultIndex};
use crate::links::LinkIndex;
#[cfg(doc)]
use crate::links::RenamePlan;
use crate::ops::{self, FileOpError};
use crate::path::VaultPath;
use crate::refresh::{Touched, VaultChange, refresh};
use crate::scan::scan;
use crate::tags::{DEFAULT_TAGS_FOLDER, TagKey, Tags};
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

/// Lo que cambió en el vault, con los índices que ya lo incluyen. `changes`
/// puede venir vacío si solo cambiaron los links o los tags: un chmod que
/// vuelve legible una nota.
#[derive(Debug, Clone)]
pub struct VaultUpdate {
    pub changes: Vec<VaultChange>,
    pub snapshot: Arc<VaultIndex>,
    pub links: Arc<LinkIndex>,
    pub tags: Arc<Tags>,
    /// Los tags cuyas notas, formas escritas o página pueden haber cambiado.
    pub tag_changes: Vec<TagKey>,
}

/// Lo que el vault puede ajustar al abrirse (03 §5, config del vault).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultOptions {
    /// La carpeta plana de las páginas de tag (05 §7).
    pub tags_folder: VaultPath,
}

impl Default for VaultOptions {
    fn default() -> Self {
        Self {
            tags_folder: VaultPath::fixed(DEFAULT_TAGS_FOLDER),
        }
    }
}

/// La raíz de un vault, ya comprobada y sin índices ni watcher: lo que basta
/// para crear una nota o buscar rutas sin leer todo el vault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultRoot(PathBuf);

impl VaultRoot {
    /// # Errors
    /// Si `root` no existe, no es una carpeta o no se puede leer.
    pub fn new(root: impl AsRef<Path>) -> Result<Self, VaultError> {
        canonical_dir(root.as_ref()).map(Self)
    }

    /// La raíz, sin enlaces simbólicos.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.0
    }

    /// Los archivos y carpetas de hoy, con las reglas de [`Vault::open`],
    /// sin leer ninguna nota: lo que necesita quien solo busca rutas.
    #[must_use]
    pub fn files(&self) -> VaultIndex {
        scan(&self.0)
    }

    /// Crea en la raíz una nota con `note` como `slug.md`, que nunca pisa
    /// otra; `now` solo nombra la que no tiene slug. Es lo que hace `nota add`.
    ///
    /// # Errors
    /// Si `note` está vacía o tiene un tag o un título inválido, o si falla la
    /// escritura.
    pub fn capture(&self, note: &NewNote, now: DateTime) -> Result<Captured, CaptureError> {
        capture::capture(&self.0, note, now)
    }
}

/// Un vault abierto: la carpeta raíz, sus índices y el watcher que los
/// mantiene al día en un hilo propio.
pub struct Vault {
    root: PathBuf,
    index: Arc<ArcSwap<VaultIndex>>,
    links: Arc<ArcSwap<LinkIndex>>,
    tags: Arc<ArcSwap<Tags>>,
    watch: Option<Watch>,
}

struct Watch {
    debouncer: Debouncer<RecommendedWatcher, RecommendedCache>,
    worker: JoinHandle<()>,
    stop: Arc<AtomicBool>,
}

impl Vault {
    /// Abre la carpeta `root` con las opciones por defecto: ver [`Vault::open_with`].
    ///
    /// # Errors
    /// Si `root` no existe, no es una carpeta o no se puede leer ni vigilar.
    pub fn open(root: impl AsRef<Path>) -> Result<(Self, mpsc::Receiver<VaultUpdate>), VaultError> {
        Self::open_with(root, VaultOptions::default())
    }

    /// Abre la carpeta `root`, indexa sus archivos y sus tags y empieza a
    /// vigilarla. Cada cambio en disco llega por el receptor como un
    /// [`VaultUpdate`]. Quien no quiera los cambios suelta el receptor y usa
    /// [`Vault::snapshot`] y [`Vault::tags`], que siguen al día igual;
    /// guardarlo sin leerlo acumula un snapshot por lote.
    ///
    /// # Errors
    /// Si `root` no existe, no es una carpeta o no se puede vigilar.
    pub fn open_with(
        root: impl AsRef<Path>,
        options: VaultOptions,
    ) -> Result<(Self, mpsc::Receiver<VaultUpdate>), VaultError> {
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
        let tags = Tags::build(options.tags_folder, &files, |path: &VaultPath| {
            read_note(&root, path)
        });
        let index = Arc::new(ArcSwap::from_pointee(files));
        let links = Arc::new(ArcSwap::from_pointee(links));
        let tags = Arc::new(ArcSwap::from_pointee(tags));
        let (updates, receiver) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let worker = Worker {
            root: root.clone(),
            index: Arc::clone(&index),
            links: Arc::clone(&links),
            tags: Arc::clone(&tags),
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
            tags,
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

    /// Los tags de este momento, sin bloquear. Como [`Vault::links`], van al
    /// día con [`Vault::snapshot`] salvo entre la publicación de uno y otro;
    /// cada [`VaultUpdate`] trae los tres del mismo lote.
    #[must_use]
    pub fn tags(&self) -> Arc<Tags> {
        self.tags.load_full()
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

    /// Aplica unos cambios armados sin tocar el disco: un renombre de nota
    /// ([`RenamePlan::changes`]) o un [`crate::tags::TagPlan`]. Comprueba
    /// antes que cada archivo diga lo que sus ediciones esperan y que los
    /// destinos estén libres, y luego edita, crea y renombra. Lo que quien
    /// llama aplique por su cuenta, como los buffers abiertos con cambios sin
    /// guardar, se quita de `changes.edits` antes. Lo devuelto sirve para
    /// [`Vault::undo`]. El índice se entera por el watcher.
    ///
    /// # Errors
    /// Si un archivo cambió o no se puede leer, o un destino está ocupado (no
    /// se escribió nada), o si algo falló a medias ([`ApplyError::Interrupted`]
    /// trae lo hecho, para deshacerlo).
    pub fn apply(&self, changes: &Changes) -> Result<Applied, ApplyError> {
        apply::apply(&self.root, changes)
    }

    /// Deshace unos cambios aplicados: cada archivo vuelve a su texto de
    /// antes, los creados se borran y los renombres se revierten. Lo que
    /// cambió después de aplicar no se toca y queda en el reporte.
    ///
    /// # Errors
    /// Si el origen de un renombre ya está ocupado: no se toca nada.
    pub fn undo(&self, applied: &Applied) -> Result<UndoReport, FileOpError> {
        apply::undo(&self.root, applied)
    }

    /// Guarda un adjunto pegado o arrastrado en `folder` (la carpeta de
    /// adjuntos del vault), creándola si falta, y devuelve su ruta y el
    /// `![[...]]` que lo embebe.
    ///
    /// # Errors
    /// Si `folder` es un archivo, pasa por un enlace simbólico o no se puede
    /// escribir.
    pub fn save_attachment(
        &self,
        folder: &VaultPath,
        attachment: &NewAttachment<'_>,
    ) -> Result<SavedAttachment, FileOpError> {
        attachment::save(&self.root, &self.snapshot(), folder, attachment)
    }

    /// Los adjuntos de `folder` que ninguna nota enlaza ni embebe. Lee todas
    /// las notas del vault: el índice de links todavía no existe.
    ///
    /// # Errors
    /// Si alguna nota no se puede leer: sin ella no se sabe qué usa.
    pub fn orphan_attachments(&self, folder: &VaultPath) -> Result<Vec<VaultPath>, FileError> {
        let index = self.snapshot();
        let mut refs = AttachmentRefs::new(&index, folder);
        for note in index.files().filter(|entry| entry.kind == EntryKind::Note) {
            // Una nota borrada mientras tanto ya no usa nada.
            if let Some(text) = file::read(&note.path.to_fs(&self.root))? {
                refs.add_note(note.path, &text);
            }
        }
        Ok(refs.orphans().into_iter().cloned().collect())
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
    tags: Arc<ArcSwap<Tags>>,
    updates: mpsc::Sender<VaultUpdate>,
    stop: Arc<AtomicBool>,
}

impl Worker {
    fn run(self, events: &mpsc::Receiver<DebounceEventResult>) {
        let mut current = VaultIndex::clone(&self.index.load());
        let mut links = LinkIndex::clone(&self.links.load());
        let mut current_tags = Tags::clone(&self.tags.load());
        let read = |path: &VaultPath| read_note(&self.root, path);
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
            for change in &changes {
                links.apply(change, |path| read_note(&self.root, path));
            }
            let mut reread = false;
            for path in &touched.reread {
                reread |= links.reread(path, |note| read_note(&self.root, note));
            }
            let mut tag_changes = current_tags.apply(&changes, &current, &read);
            // Un chmod no cambia el índice de archivos, pero puede volver legible una nota.
            let rescans: Vec<VaultChange> = touched
                .reread
                .iter()
                .map(|path| VaultChange::Modified { path: path.clone() })
                .collect();
            tag_changes.extend(current_tags.apply(&rescans, &current, &read));
            tag_changes.sort_unstable();
            tag_changes.dedup();
            if changes.is_empty() && !reread && tag_changes.is_empty() {
                continue;
            }
            let snapshot = Arc::new(current.clone());
            let published = Arc::new(links.clone());
            let tags = Arc::new(current_tags.clone());
            self.index.store(Arc::clone(&snapshot));
            self.links.store(Arc::clone(&published));
            self.tags.store(Arc::clone(&tags));
            // Si el receptor ya no existe, quien abrió el vault solo usa los snapshots.
            let _ = self.updates.send(VaultUpdate {
                changes,
                snapshot,
                links: published,
                tags,
                tag_changes,
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

/// El contenido de una nota para los índices de links y de tags; si no se
/// puede leer, queda sin links ni tags hasta su próximo cambio.
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
    if !canonical.is_dir() {
        return Err(VaultError::NotADirectory(root.to_owned()));
    }
    // Sin esto, una raíz sin permiso de lectura se abriría como un vault vacío.
    fs::read_dir(&canonical).map_err(|source| VaultError::Open {
        root: root.to_owned(),
        source,
    })?;
    Ok(canonical)
}
