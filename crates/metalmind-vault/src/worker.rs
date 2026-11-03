//! El hilo de un vault abierto: mantiene el índice al día con lo que ve el
//! watcher y es el único que escribe el historial, así que las versiones y
//! los renombres le llegan en el orden en que pasaron.

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, mpsc};
use std::time::SystemTime;

use arc_swap::ArcSwap;

use crate::debounce::Batch;
use crate::file;
use crate::history::{History, HistoryError, VersionId};
use crate::inbox::Inbox;
use crate::index::{EntryKind, VaultIndex};
use crate::links::LinkIndex;
use crate::note_batch::NoteBatch;
use crate::note_cache::CacheRefresh;
use crate::path::VaultPath;
use crate::refresh::{Touched, VaultChange, refresh};
use crate::tags::{TagKey, Tags};
use crate::vault::{SaveKind, VaultUpdate};
use crate::watch::{touched, touched_by_errors};
use crate::watcher::{Watch, before_reading};

pub(crate) enum Message {
    Disk(Batch),
    Save(Save),
    /// Avisa cuando ya se registraron los guardados que llegaron antes.
    Flush(mpsc::Sender<()>),
    Stop,
}

/// Una nota que ya quedó en disco y cuya versión falta registrar.
pub(crate) struct Save {
    pub path: VaultPath,
    pub text: String,
    pub at: SystemTime,
    pub kind: SaveKind,
    pub done: mpsc::Sender<Result<Option<VersionId>, HistoryError>>,
}

pub(crate) struct Worker {
    pub root: PathBuf,
    pub index: Arc<ArcSwap<VaultIndex>>,
    pub links: Arc<ArcSwap<LinkIndex>>,
    pub tags: Arc<ArcSwap<Tags>>,
    /// `None` si el vault no tiene historial.
    pub history: Option<Arc<History>>,
    /// Lo que hay que reescribir de la caché, si cambió algo desde la última vez.
    pub cache_refresh: Option<CacheRefresh>,
    /// Lo toma `Vault::save` mientras escribe y manda su versión: con él
    /// tomado, lo que haya en disco es posterior a todo guardado ya enviado.
    pub saving: Arc<Mutex<()>>,
    pub updates: mpsc::Sender<VaultUpdate>,
    pub stop: Arc<AtomicBool>,
    /// Lo vigilado, que amplía cada walk.
    pub watch: Arc<Mutex<Watch>>,
}

impl Worker {
    pub(crate) fn run(self, messages: &mpsc::Receiver<Message>) {
        let mut indexes = Indexes {
            files: VaultIndex::clone(&self.index.load()),
            links: LinkIndex::clone(&self.links.load()),
            tags: Tags::clone(&self.tags.load()),
        };
        if let Some(refresh) = &self.cache_refresh {
            refresh.write(&indexes.links, &indexes.tags);
        }
        self.open_history(&indexes.files);
        let mut inbox = Inbox::new(messages);
        while let Some(message) = inbox.next() {
            match message {
                Message::Disk(first) => self.on_disk(&mut indexes, &first, &mut inbox),
                Message::Save(save) => self.record(&save),
                // Quien espera puede haberse ido.
                Message::Flush(done) => drop(done.send(())),
                Message::Stop => break,
            }
        }
        self.watch().stop();
    }

    /// Relee de una vez los eventos de disco seguidos que esperan.
    fn on_disk(&self, indexes: &mut Indexes, first: &Batch, inbox: &mut Inbox) {
        let mut touched = self.touched(first);
        while let Some(more) = inbox.next_disk() {
            touched.merge(self.touched(&more));
        }
        if self.stop.load(Ordering::Acquire) {
            return;
        }
        let read = |path: &VaultPath| read_note(&self.root, path);
        self.on_touched(indexes, &touched, &read, inbox);
    }

    /// Lleva índices e historial a lo que dice `touched`, y lo publica. `read`
    /// es la única puerta al contenido de las notas en todo el lote.
    fn on_touched(
        &self,
        indexes: &mut Indexes,
        touched: &Touched,
        read: &dyn Fn(&VaultPath) -> Option<String>,
        inbox: &mut Inbox,
    ) {
        let watching = self.watch().lost().is_none();
        let changes = self.refresh(&mut indexes.files, touched);
        let lost = watching && self.watch().lost().is_some();
        let mut notes = NoteBatch::new(read);
        let external: Vec<VaultPath> = changes.iter().filter_map(edited_note).cloned().collect();
        let saves = self.read_external(&external, &mut notes, inbox);
        let (reread, tag_changes) = indexes.apply(&changes, touched, &mut notes);
        // Perder la vigilancia también se publica: sin eso no llegaría otro lote.
        if changes.is_empty() && !reread && tag_changes.is_empty() && !lost {
            return;
        }
        // Antes de publicar, para que quien reciba el cambio ya vea el
        // historial en la ruta nueva.
        let saves = self.follow_disk_changes(&changes, saves, inbox);
        self.publish(indexes, changes, tag_changes);
        for save in saves {
            self.record(&save);
        }
        self.record_external(notes.into_texts(&external));
    }

    fn publish(&self, indexes: &Indexes, changes: Vec<VaultChange>, tag_changes: Vec<TagKey>) {
        let snapshot = Arc::new(indexes.files.clone());
        let published = Arc::new(indexes.links.clone());
        let published_tags = Arc::new(indexes.tags.clone());
        self.index.store(Arc::clone(&snapshot));
        self.links.store(Arc::clone(&published));
        self.tags.store(Arc::clone(&published_tags));
        // Si el receptor ya no existe, quien abrió el vault solo usa los snapshots.
        let _ = self.updates.send(VaultUpdate {
            changes,
            snapshot,
            links: published,
            tags: published_tags,
            tag_changes,
        });
    }

    /// Sigue los renombres y borrados del lote. Antes registra los guardados
    /// ya escritos en una ruta que el lote renombra o borra: se escribieron
    /// antes del cambio, y si no su versión quedaría en la ruta vieja. Devuelve
    /// los demás de `saves`, como uno en el destino de un renombre: van después.
    fn follow_disk_changes(
        &self,
        changes: &[VaultChange],
        saves: Vec<Save>,
        inbox: &mut Inbox,
    ) -> Vec<Save> {
        let gone: Vec<&VaultPath> = changes
            .iter()
            .filter_map(|change| match change {
                VaultChange::Renamed { from, .. } => Some(from),
                VaultChange::Removed { path, .. } => Some(path),
                _ => None,
            })
            .collect();
        let in_gone = |save: &Save| gone.iter().any(|path| save.path.starts_with(path));
        let (mut before, after): (Vec<Save>, Vec<Save>) =
            saves.into_iter().partition(|save| in_gone(save));
        {
            let _saving = self.saving.lock().unwrap_or_else(PoisonError::into_inner);
            before.extend(inbox.pending_saves(in_gone));
        }
        for save in before {
            self.record(&save);
        }
        self.follow_in_history(changes);
        after
    }

    /// Solo llegan guardados si hay historial.
    fn record(&self, save: &Save) {
        let Some(history) = &self.history else {
            return;
        };
        let result = match save.kind {
            SaveKind::Explicit => history.record(&save.path, &save.text, save.at),
            SaveKind::Autosave => history.record_autosave(&save.path, &save.text, save.at),
        };
        if let Err(error) = &result {
            log::warn!("{error}");
        }
        // Quien guardó puede no esperar el resultado.
        let _ = save.done.send(result);
    }

    /// Lee `external` para el historial y devuelve los guardados ya escritos,
    /// que van antes: el disco tiene algo igual o más nuevo que ellos. Lo que
    /// se guarde después llega al canal y queda tras lo leído aquí.
    fn read_external(
        &self,
        external: &[VaultPath],
        notes: &mut NoteBatch<'_>,
        inbox: &mut Inbox,
    ) -> Vec<Save> {
        if self.history.is_none() || external.is_empty() {
            return Vec::new();
        }
        // Solo mientras se lee: registrar puede tardar y un guardado esperaría.
        let _saving = self.saving.lock().unwrap_or_else(PoisonError::into_inner);
        for path in external {
            notes.text(path);
        }
        inbox.pending_saves(|_| true)
    }

    /// Lo que cambió fuera de Metalmind también es una versión (06 §3). Un
    /// guardado propio llega aquí igual, pero ya es la última versión y no
    /// crea otra.
    fn record_external(&self, texts: Vec<(VaultPath, String)>) {
        let Some(history) = &self.history else {
            return;
        };
        if let Err(error) = history.record_all(texts, SystemTime::now()) {
            log::warn!("{error}");
        }
    }

    /// Al abrir el vault, el historial se alinea con lo que pasó con el vault
    /// cerrado y se aplica la retención. Va aquí y no en `open`: los dos leen
    /// del disco y no tienen por qué demorar la apertura. Lo que llegue
    /// mientras tanto espera en el canal, así que nada se registra antes.
    fn open_history(&self, files: &VaultIndex) {
        let Some(history) = &self.history else {
            return;
        };
        let notes: HashSet<&VaultPath> = files
            .files()
            .filter(|entry| entry.kind == EntryKind::Note)
            .map(|entry| entry.path)
            .collect();
        // Un historial que no se puede alinear o podar no impide usar el vault.
        if let Err(error) = history.reconcile(&self.root, &notes, SystemTime::now()) {
            log::warn!("{error}");
        }
        // Si el vault ya se cierra (la app sale, o no llegó a abrir), la poda
        // espera a la próxima apertura en vez de demorar la salida.
        if self.stop.load(Ordering::Acquire) {
            return;
        }
        let running = || !self.stop.load(Ordering::Acquire);
        if let Err(error) = history.prune_while(SystemTime::now(), running) {
            log::warn!("{error}");
        }
    }

    fn follow_in_history(&self, changes: &[VaultChange]) {
        let Some(history) = &self.history else {
            return;
        };
        if let Err(error) = history.follow(&self.root, changes, SystemTime::now()) {
            log::warn!("{error}");
        }
    }

    /// Relee `touched`, vigilando cada carpeta antes de leerla, y deja de
    /// vigilar lo que salió del índice. Si apareció `.git/info`, relee todo:
    /// su `exclude` no dio evento.
    fn refresh(&self, files: &mut VaultIndex, touched: &Touched) -> Vec<VaultChange> {
        let before = before_reading(&self.watch);
        let mut changes = refresh(files, &self.root, touched, Some(Arc::clone(&before)));
        while self.watch().follow(files) {
            let all = Touched {
                paths: BTreeSet::from([VaultPath::root()]),
                ..Touched::default()
            };
            changes.extend(refresh(files, &self.root, &all, Some(Arc::clone(&before))));
        }
        changes
    }

    fn watch(&self) -> MutexGuard<'_, Watch> {
        self.watch.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn touched(&self, batch: &Batch) -> Touched {
        for error in &batch.errors {
            log::warn!("watcher de {}: {error}", self.root.display());
        }
        let mut touched = touched(&self.root, &batch.events);
        touched.merge(touched_by_errors(&self.root, &batch.errors));
        touched
    }
}

/// El contenido de una nota para los índices de links y de tags; si no se
/// puede leer, queda sin links ni tags hasta su próximo cambio.
pub(crate) fn read_note(root: &Path, path: &VaultPath) -> Option<String> {
    match file::read(&path.to_fs(root)) {
        Ok(text) => text,
        Err(error) => {
            log::warn!("{error}");
            None
        }
    }
}

fn edited_note(change: &VaultChange) -> Option<&VaultPath> {
    match change {
        VaultChange::Created {
            path,
            kind: EntryKind::Note,
        } => Some(path),
        VaultChange::Modified { path } if EntryKind::of_file(path) == EntryKind::Note => Some(path),
        // Lo que tenga ahora: un cambio de antes del renombre puede llegar
        // después nombrando la ruta vieja, que ya no existe.
        VaultChange::Renamed {
            to,
            kind: EntryKind::Note,
            ..
        } => Some(to),
        _ => None,
    }
}

/// Lo que el hilo mantiene al día y publica en cada lote.
struct Indexes {
    files: VaultIndex,
    links: LinkIndex,
    tags: Tags,
}

impl Indexes {
    /// Pone links y tags al día con `files`, ya actualizado. Devuelve si un
    /// chmod cambió algún link y las claves de tag que cambiaron.
    fn apply(
        &mut self,
        changes: &[VaultChange],
        touched: &Touched,
        notes: &mut NoteBatch<'_>,
    ) -> (bool, Vec<TagKey>) {
        for change in changes {
            self.links.apply(change, notes);
        }
        let mut reread = false;
        for path in &touched.reread {
            reread |= self.links.reread(path, notes);
        }
        let mut tag_changes = self.tags.apply(changes, &self.files, notes);
        // Un chmod no cambia el índice de archivos, pero puede volver legible una nota.
        let rescans: Vec<VaultChange> = touched
            .reread
            .iter()
            .map(|path| VaultChange::Modified { path: path.clone() })
            .collect();
        tag_changes.extend(self.tags.apply(&rescans, &self.files, notes));
        tag_changes.sort_unstable();
        tag_changes.dedup();
        (reread, tag_changes)
    }
}

#[cfg(test)]
#[path = "worker_tests.rs"]
mod tests;
