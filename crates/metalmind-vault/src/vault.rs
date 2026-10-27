use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime};

use arc_swap::ArcSwap;
use jiff::civil::DateTime;
use notify_debouncer_full::new_debouncer;

use crate::apply::{self, Applied, ApplyError, UndoReport};
use crate::attachment::{
    self, AttachmentError, DEFAULT_ATTACHMENTS_FOLDER, NewAttachment, SavedAttachment,
};
use crate::capture::{self, Capture, CaptureError, Captured, NewNote};
use crate::changes::Changes;
use crate::file::{FileError, create_atomic, write_atomic};
use crate::history::{History, HistoryError, Retention, VersionId};
use crate::index::VaultIndex;
use crate::links::LinkIndex;
#[cfg(doc)]
use crate::links::RenamePlan;
use crate::ops::{self, FileOpError};
use crate::path::VaultPath;
use crate::refresh::VaultChange;
use crate::scan::{scan, scan_within};
use crate::tags::{DEFAULT_TAGS_FOLDER, TagKey, Tags};
use crate::watcher::{FolderWatch, Unwatched, before_reading};
use crate::worker::{Message, Save, Worker, read_note};

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

#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error(transparent)]
    Path(#[from] FileOpError),
    #[error(transparent)]
    Write(#[from] FileError),
}

/// La nota ya se guardó; lo que falló es registrar su versión.
#[derive(Debug, thiserror::Error)]
pub enum VersionError {
    #[error("la nota se guardó, pero no su versión: {0}")]
    History(#[from] HistoryError),
    #[error("la nota se guardó, pero el vault se cerró antes de registrar su versión")]
    Stopped,
}

/// Quién pidió un guardado: el historial agrupa las versiones del
/// autoguardado y deja siempre la de un guardado pedido (06 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveKind {
    Explicit,
    Autosave,
}

/// La versión de un guardado, que se registra en el hilo del vault para no
/// demorar el guardado. Se puede soltar sin esperarla: un error igual queda
/// en el log.
#[derive(Debug)]
pub struct PendingVersion(mpsc::Receiver<Result<Option<VersionId>, HistoryError>>);

impl PendingVersion {
    /// Espera a que se registre. `None` si el texto era igual a la última
    /// versión y no hizo falta otra.
    ///
    /// # Errors
    /// Si no se pudo escribir el historial o el vault se cerró antes.
    pub fn wait(self) -> Result<Option<VersionId>, VersionError> {
        Ok(self.0.recv().map_err(|_| VersionError::Stopped)??)
    }
}

/// Lo que cambió en el vault, con los índices que ya lo incluyen. `changes`
/// puede venir vacío si solo cambiaron los links o los tags (un chmod que
/// vuelve legible una nota) o si se dejó de vigilar el vault
/// ([`Vault::unwatched`]).
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
    /// Donde van los adjuntos pegados o arrastrados (06 §6).
    pub attachments_folder: VaultPath,
    /// Dónde y cuánto historial de versiones guardar (06 §3); sin esto, el
    /// vault no tiene historial.
    pub history: Option<HistoryOptions>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryOptions {
    /// La carpeta del historial, fuera del vault (06 §3); se crea al primer
    /// guardado.
    pub dir: PathBuf,
    pub retention: Retention,
}

impl Default for VaultOptions {
    fn default() -> Self {
        Self {
            tags_folder: VaultPath::fixed(DEFAULT_TAGS_FOLDER),
            attachments_folder: VaultPath::fixed(DEFAULT_ATTACHMENTS_FOLDER),
            history: None,
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
        scan(&self.0, None)
    }

    /// Lo que hay en `folders` y sus subcarpetas, con las reglas de
    /// [`Vault::open`], sin recorrer el resto del vault ni leer ninguna nota:
    /// lo que necesita quien solo mira las plantillas o las páginas de tag.
    #[must_use]
    pub fn files_within(&self, folders: &[VaultPath]) -> VaultIndex {
        scan_within(&self.0, folders)
    }

    /// Crea en la raíz una nota con `note` como `slug.md`, que nunca pisa
    /// otra; `now` solo nombra la que no tiene slug. Es lo que hace `metalmind add`.
    ///
    /// # Errors
    /// Si `note` está vacía o tiene un tag o un título inválido, o si falla la
    /// escritura.
    pub fn capture(&self, note: &NewNote, now: DateTime) -> Result<Captured, CaptureError> {
        capture::capture(&self.0, note, now)
    }

    /// Como [`VaultRoot::capture`], pero el texto de la nota lo arma `fill`
    /// con lo que la captura decidió: los tags limpios, el título y el
    /// nombre. Es lo que hace `metalmind add` con una plantilla.
    ///
    /// # Errors
    /// Si `note` tiene un tag o un título inválido, o si falla la escritura.
    pub fn capture_with(
        &self,
        note: &NewNote,
        now: DateTime,
        fill: impl FnOnce(Capture<'_>) -> String,
    ) -> Result<Captured, CaptureError> {
        capture::capture_with(&self.0, note, now, |capture| Ok(fill(capture)))
    }
}

/// Un vault abierto: la carpeta raíz, sus índices y el watcher que los
/// mantiene al día en un hilo propio.
pub struct Vault {
    root: PathBuf,
    attachments_folder: VaultPath,
    index: Arc<ArcSwap<VaultIndex>>,
    links: Arc<ArcSwap<LinkIndex>>,
    tags: Arc<ArcSwap<Tags>>,
    history: Option<Arc<History>>,
    messages: mpsc::Sender<Message>,
    saving: Arc<Mutex<()>>,
    unwatched: Arc<OnceLock<Unwatched>>,
    worker: Option<WorkerThread>,
}

struct WorkerThread {
    handle: JoinHandle<()>,
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
    /// Con `options.history`, el historial sigue por su cuenta los renombres
    /// y borrados que ve el watcher, y guarda como versión lo que cambie una
    /// nota desde fuera.
    ///
    /// Si no quedan watches de inotify, el vault abre igual pero no ve los
    /// cambios de fuera: ver [`Vault::unwatched`].
    ///
    /// # Errors
    /// Si `root` no existe, no es una carpeta o no se puede vigilar.
    pub fn open_with(
        root: impl AsRef<Path>,
        options: VaultOptions,
    ) -> Result<(Self, mpsc::Receiver<VaultUpdate>), VaultError> {
        let root = canonical_dir(root.as_ref())?;
        let (messages, inbox) = mpsc::channel();
        let disk = messages.clone();
        let debouncer = new_debouncer(DEBOUNCE, None, move |result| {
            // Si el hilo ya terminó, el vault se está cerrando.
            let _ = disk.send(Message::Disk(result));
        })?;
        let unwatched = Arc::new(OnceLock::new());
        // La raíz antes del walk: lo que cambie mientras tanto llega como evento.
        let watch = FolderWatch::start(&root, debouncer, Arc::clone(&unwatched))?;
        let watch = Arc::new(Mutex::new(watch));
        let files = scan(&root, Some(before_reading(&watch)));
        let links = LinkIndex::build(
            files.files().map(|entry| (entry.path.clone(), entry.kind)),
            |path| read_note(&root, path),
        );
        let tags = Tags::build(options.tags_folder, &files, |path: &VaultPath| {
            read_note(&root, path)
        });
        let history = options
            .history
            .map(|options| Arc::new(History::new(options.dir, options.retention)));
        let index = Arc::new(ArcSwap::from_pointee(files));
        let links = Arc::new(ArcSwap::from_pointee(links));
        let tags = Arc::new(ArcSwap::from_pointee(tags));
        let (updates, receiver) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let saving = Arc::new(Mutex::new(()));
        let worker = Worker {
            root: root.clone(),
            index: Arc::clone(&index),
            links: Arc::clone(&links),
            tags: Arc::clone(&tags),
            history: history.as_ref().map(Arc::clone),
            saving: Arc::clone(&saving),
            updates,
            stop: Arc::clone(&stop),
            watch,
        };
        let handle = thread::Builder::new()
            .name("metalmind-vault".to_owned())
            .spawn(move || worker.run(&inbox))
            .map_err(VaultError::Spawn)?;
        let vault = Self {
            root,
            attachments_folder: options.attachments_folder,
            index,
            links,
            tags,
            history,
            messages,
            saving,
            unwatched,
            worker: Some(WorkerThread { handle, stop }),
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

    /// Por qué el vault dejó de ver los cambios que se hacen fuera de
    /// Metalmind, si ya no los ve. Puede pasar al abrirlo o después, al
    /// aparecer carpetas; no tiene vuelta.
    #[must_use]
    pub fn unwatched(&self) -> Option<Unwatched> {
        self.unwatched.get().copied()
    }

    /// El historial de versiones de las notas de este vault, si tiene.
    #[must_use]
    pub fn history(&self) -> Option<&History> {
        self.history.as_deref()
    }

    /// Guarda `text` en la nota `path` de forma atómica y registra la
    /// versión, según `kind`: toda escritura de una nota del vault pasa por
    /// aquí o por [`Vault::create`] (06 §3). Vuelve en cuanto la nota está en disco; la versión se
    /// registra después, en el hilo del vault. `None` si el vault no tiene
    /// historial.
    ///
    /// # Errors
    /// Si `path` no es una nota o pasa por un enlace simbólico, o si falla la
    /// escritura.
    pub fn save(
        &self,
        path: &VaultPath,
        text: String,
        kind: SaveKind,
    ) -> Result<Option<PendingVersion>, SaveError> {
        self.store(path, text, kind, |target, text| {
            write_atomic(target, |out| out.write_all(text.as_bytes()))
        })
    }

    /// Como [`Vault::save`], pero solo crea la nota: si ya existe, da
    /// [`FileError::Exists`] y no la toca, también si aparece mientras se
    /// escribe.
    ///
    /// # Errors
    /// Si ya existe, `path` no es una nota o pasa por un enlace simbólico, o
    /// si falla la escritura.
    pub fn create(
        &self,
        path: &VaultPath,
        text: String,
        kind: SaveKind,
    ) -> Result<Option<PendingVersion>, SaveError> {
        self.store(path, text, kind, |target, text| {
            create_atomic(target, |out| out.write_all(text.as_bytes()))
        })
    }

    /// Escribe `text` en la nota con `write` y manda su versión al hilo.
    fn store(
        &self,
        path: &VaultPath,
        text: String,
        kind: SaveKind,
        write: impl FnOnce(&Path, &str) -> Result<(), FileError>,
    ) -> Result<Option<PendingVersion>, SaveError> {
        let target = ops::note_target(&self.root, path)?;
        if self.history.is_none() {
            write(&target, &text)?;
            return Ok(None);
        }
        // Hasta mandar la versión: el hilo no lee el disco mientras tanto.
        let _saving = self.saving.lock().unwrap_or_else(PoisonError::into_inner);
        write(&target, &text)?;
        let (done, pending) = mpsc::channel();
        // Si el hilo terminó en pánico, `PendingVersion::wait` lo dice.
        let _ = self.messages.send(Message::Save(Save {
            path: path.clone(),
            text,
            at: SystemTime::now(),
            kind,
            done,
        }));
        Ok(Some(PendingVersion(pending)))
    }

    /// Antes de salir de la app: el hilo deja de releer el disco y de
    /// mantener el historial (alinearlo o podarlo espera a la próxima
    /// apertura) y se espera a que registre las versiones de los guardados
    /// que ya volvieron, para que lo guardado justo antes de salir no se
    /// quede sin versión. Después de esto, el vault solo sirve para soltarlo.
    pub fn shut_down(&self) {
        let Some(worker) = &self.worker else {
            return;
        };
        worker.stop.store(true, Ordering::Release);
        let (done, flushed) = mpsc::channel();
        if self.messages.send(Message::Flush(done)).is_ok() {
            // Si el hilo terminó, no queda nada que esperar.
            let _ = flushed.recv();
        }
    }

    /// Crea una nota con `text`, de una vez y con las carpetas que le
    /// falten; nunca pisa una que exista, aunque aparezca mientras se
    /// escribe. El índice se entera por el watcher, como de cualquier otro
    /// cambio en disco.
    ///
    /// # Errors
    /// Si no termina en `.md`, ya existe o pasa por un enlace simbólico.
    pub fn create_note(&self, path: &VaultPath, text: &str) -> Result<(), FileOpError> {
        ops::create_note(&self.root, path, text)
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
    /// [`Vault::undo`]. El índice y el historial se enteran por el watcher.
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

    /// Guarda un adjunto pegado o arrastrado en la carpeta de adjuntos,
    /// creándola si falta, y devuelve su ruta y el `![[...]]` con que lo
    /// embebe su nota. El índice se entera por el watcher.
    ///
    /// # Errors
    /// Si el archivo sería una nota, la carpeta es un archivo o pasa por un
    /// enlace simbólico, el vault lo ignoraría, un wikilink no puede
    /// nombrarlo o falla la escritura. En todos los casos no queda nada.
    pub fn save_attachment(
        &self,
        attachment: &NewAttachment<'_>,
    ) -> Result<SavedAttachment, AttachmentError> {
        attachment::save(
            &self.root,
            &self.links(),
            &self.attachments_folder,
            attachment,
        )
    }

    /// Los adjuntos de la carpeta de adjuntos que ninguna nota usa, según
    /// los links de este momento: ver [`LinkIndex::orphan_attachments`].
    #[must_use]
    pub fn orphan_attachments(&self) -> Vec<VaultPath> {
        self.links().orphan_attachments(&self.attachments_folder)
    }
}

impl Drop for Vault {
    fn drop(&mut self) {
        let Some(WorkerThread { handle, stop }) = self.worker.take() else {
            return;
        };
        // El hilo deja de releer el disco, pero registra las versiones que
        // esperan antes de salir; después para el watcher.
        stop.store(true, Ordering::Release);
        // Si el hilo ya terminó en pánico, lo dice el `join`.
        let _ = self.messages.send(Message::Stop);
        if handle.join().is_err() {
            log::error!("el watcher de {} terminó en pánico", self.root.display());
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
