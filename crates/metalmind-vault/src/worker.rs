//! El hilo de un vault abierto: mantiene el índice al día con lo que ve el
//! watcher y es el único que escribe el historial, así que las versiones y
//! los renombres le llegan en el orden en que pasaron.

use std::collections::{BTreeSet, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, mpsc};
use std::time::SystemTime;

use arc_swap::ArcSwap;
use notify_debouncer_full::DebounceEventResult;

use crate::file;
use crate::history::{History, HistoryError, VersionId};
use crate::index::{EntryKind, VaultIndex};
use crate::links::LinkIndex;
use crate::path::VaultPath;
use crate::refresh::{Touched, VaultChange, refresh};
use crate::tags::Tags;
use crate::vault::{SaveKind, VaultUpdate};
use crate::watch::{touched, touched_by_errors};
use crate::watcher::{Watch, before_reading};

pub(crate) enum Message {
    Disk(DebounceEventResult),
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
        self.open_history(&indexes.files);
        let mut inbox = Inbox::new(messages);
        while let Some(message) = inbox.next() {
            match message {
                Message::Disk(first) => self.on_disk(&mut indexes, first, &mut inbox),
                Message::Save(save) => self.record(&save),
                // Quien espera puede haberse ido.
                Message::Flush(done) => drop(done.send(())),
                Message::Stop => break,
            }
        }
        self.watch().stop();
    }

    /// Relee de una vez los eventos de disco seguidos que esperan.
    fn on_disk(&self, indexes: &mut Indexes, first: DebounceEventResult, inbox: &mut Inbox) {
        let mut touched = self.touched(first);
        while let Some(more) = inbox.next_disk() {
            touched.merge(self.touched(more));
        }
        if self.stop.load(Ordering::Acquire) {
            return;
        }
        let Indexes { files, links, tags } = indexes;
        let read = |path: &VaultPath| read_note(&self.root, path);
        let watching = self.watch().lost().is_none();
        let changes = self.refresh(files, &touched);
        let lost = watching && self.watch().lost().is_some();
        for change in &changes {
            links.apply(change, read);
        }
        let mut reread = false;
        for path in &touched.reread {
            reread |= links.reread(path, read);
        }
        let mut tag_changes = tags.apply(&changes, files, &read);
        // Un chmod no cambia el índice de archivos, pero puede volver legible una nota.
        let rescans: Vec<VaultChange> = touched
            .reread
            .iter()
            .map(|path| VaultChange::Modified { path: path.clone() })
            .collect();
        tag_changes.extend(tags.apply(&rescans, files, &read));
        tag_changes.sort_unstable();
        tag_changes.dedup();
        // Perder la vigilancia también se publica: sin eso no llegaría otro lote.
        if changes.is_empty() && !reread && tag_changes.is_empty() && !lost {
            return;
        }
        // Antes de publicar, para que quien reciba el cambio ya vea el
        // historial en la ruta nueva.
        self.follow_disk_changes(&changes, inbox);
        let snapshot = Arc::new(files.clone());
        let published = Arc::new(links.clone());
        let published_tags = Arc::new(tags.clone());
        self.index.store(Arc::clone(&snapshot));
        self.links.store(Arc::clone(&published));
        self.tags.store(Arc::clone(&published_tags));
        let external: Vec<VaultPath> = changes.iter().filter_map(edited_note).cloned().collect();
        // Si el receptor ya no existe, quien abrió el vault solo usa los snapshots.
        let _ = self.updates.send(VaultUpdate {
            changes,
            snapshot,
            links: published,
            tags: published_tags,
            tag_changes,
        });
        self.record_external(&external, inbox);
    }

    /// Sigue los renombres y borrados del lote. Antes registra los guardados
    /// ya escritos en una ruta que el lote renombra o borra: se escribieron
    /// antes del cambio, y si no su versión quedaría en la ruta vieja. Los
    /// demás, como uno en el destino de un renombre, van después y esperan.
    fn follow_disk_changes(&self, changes: &[VaultChange], inbox: &mut Inbox) {
        let gone: Vec<&VaultPath> = changes
            .iter()
            .filter_map(|change| match change {
                VaultChange::Renamed { from, .. } => Some(from),
                VaultChange::Removed { path, .. } => Some(path),
                _ => None,
            })
            .collect();
        let saves = {
            let _saving = self.saving.lock().unwrap_or_else(PoisonError::into_inner);
            inbox.pending_saves(|save| gone.iter().any(|path| save.path.starts_with(path)))
        };
        for save in saves {
            self.record(&save);
        }
        self.follow_in_history(changes);
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

    /// Lo que cambió fuera de Metalmind también es una versión (06 §3). Un
    /// guardado propio llega aquí igual, pero ya es la última versión y no
    /// crea otra. Los guardados ya escritos se registran antes de leer el
    /// disco, que tiene algo igual o más nuevo que ellos.
    fn record_external(&self, notes: &[VaultPath], inbox: &mut Inbox) {
        let Some(history) = &self.history else {
            return;
        };
        if notes.is_empty() {
            return;
        }
        // Solo mientras se lee: registrar puede tardar y un guardado esperaría.
        let (saves, texts) = {
            let _saving = self.saving.lock().unwrap_or_else(PoisonError::into_inner);
            (inbox.pending_saves(|_| true), self.read_notes(notes))
        };
        for save in saves {
            self.record(&save);
        }
        if let Err(error) = history.record_all(texts, SystemTime::now()) {
            log::warn!("{error}");
        }
    }

    fn read_notes(&self, notes: &[VaultPath]) -> Vec<(VaultPath, String)> {
        notes
            .iter()
            .filter_map(|path| Some((path.clone(), read_note(&self.root, path)?)))
            .collect()
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

/// Los mensajes del hilo, con los que ya se sacaron del canal y esperan su
/// turno en orden.
struct Inbox<'a> {
    receiver: &'a mpsc::Receiver<Message>,
    queue: VecDeque<Message>,
}

impl<'a> Inbox<'a> {
    fn new(receiver: &'a mpsc::Receiver<Message>) -> Self {
        Self {
            receiver,
            queue: VecDeque::new(),
        }
    }

    fn next(&mut self) -> Option<Message> {
        self.queue.pop_front().or_else(|| self.receiver.recv().ok())
    }

    /// El siguiente evento de disco si es lo próximo que espera, sin bloquear.
    fn next_disk(&mut self) -> Option<DebounceEventResult> {
        let message = match self.queue.pop_front() {
            Some(message) => message,
            None => self.receiver.try_recv().ok()?,
        };
        match message {
            Message::Disk(result) => Some(result),
            other => {
                self.queue.push_front(other);
                None
            }
        }
    }

    /// Los guardados que ya llegaron, en orden; el resto sigue esperando.
    fn pending_saves(&mut self, wanted: impl Fn(&Save) -> bool) -> Vec<Save> {
        self.queue.extend(self.receiver.try_iter());
        let mut saves = Vec::new();
        let mut rest = VecDeque::with_capacity(self.queue.len());
        for message in self.queue.drain(..) {
            match message {
                Message::Save(save) if wanted(&save) => saves.push(save),
                other => rest.push_back(other),
            }
        }
        self.queue = rest;
        saves
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::Duration;

    use notify_debouncer_full::new_debouncer;
    use tempfile::TempDir;

    use super::*;
    use crate::history::TEST_RETENTION;
    use crate::watcher::FolderWatch;

    fn worker(root: &Path, history: &Arc<History>) -> Worker {
        let (updates, _published) = mpsc::channel();
        Worker {
            root: root.to_owned(),
            index: Arc::new(ArcSwap::from_pointee(VaultIndex::default())),
            links: Arc::new(ArcSwap::from_pointee(LinkIndex::default())),
            tags: Arc::new(ArcSwap::from_pointee(Tags::build(
                VaultPath::fixed(crate::tags::DEFAULT_TAGS_FOLDER),
                &VaultIndex::default(),
                |_: &VaultPath| None,
            ))),
            history: Some(Arc::clone(history)),
            saving: Arc::new(Mutex::new(())),
            updates,
            stop: Arc::new(AtomicBool::new(false)),
            watch: Arc::new(Mutex::new(watch(root))),
        }
    }

    fn watch(root: &Path) -> Watch {
        let debouncer = new_debouncer(Duration::from_millis(10), None, |_| {}).unwrap();
        FolderWatch::start(root, debouncer, Arc::default()).unwrap()
    }

    fn contents(history: &History, note: &VaultPath) -> Vec<String> {
        history
            .versions(note)
            .unwrap()
            .iter()
            .map(|version| history.content(note, version.id).unwrap())
            .collect()
    }

    #[test]
    fn un_guardado_ya_escrito_se_registra_antes_de_seguir_un_renombre() {
        let (root, data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
        let history = Arc::new(History::new(data.path(), TEST_RETENTION));
        let worker = worker(root.path(), &history);
        let (a, b) = (
            VaultPath::new("a.md").unwrap(),
            VaultPath::new("b.md").unwrap(),
        );
        history.record(&a, "uno", SystemTime::now()).unwrap();
        // Se guardó "dos" en `a.md` y después se renombró, pero el hilo ve el
        // renombre antes que el mensaje del guardado.
        let (messages, received) = mpsc::channel();
        let (done, _recorded) = mpsc::channel();
        messages
            .send(Message::Save(Save {
                path: a.clone(),
                text: "dos".to_owned(),
                at: SystemTime::now(),
                kind: SaveKind::Explicit,
                done,
            }))
            .unwrap();
        let changes = [VaultChange::Renamed {
            from: a,
            to: b.clone(),
            kind: EntryKind::Note,
        }];
        worker.follow_disk_changes(&changes, &mut Inbox::new(&received));
        assert_eq!(contents(&history, &b), ["uno", "dos"]);
    }

    #[test]
    fn un_guardado_en_el_destino_de_un_renombre_espera_al_renombre() {
        let (root, data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
        let history = Arc::new(History::new(data.path(), TEST_RETENTION));
        let worker = worker(root.path(), &history);
        let (a, b) = (
            VaultPath::new("a.md").unwrap(),
            VaultPath::new("b.md").unwrap(),
        );
        history.record(&a, "uno", SystemTime::now()).unwrap();
        // Renombrada a `b.md` y guardada ahí: el guardado es posterior.
        let (messages, received) = mpsc::channel();
        let (done, recorded) = mpsc::channel();
        messages
            .send(Message::Save(Save {
                path: b.clone(),
                text: "dos".to_owned(),
                at: SystemTime::now(),
                kind: SaveKind::Explicit,
                done,
            }))
            .unwrap();
        let changes = [VaultChange::Renamed {
            from: a,
            to: b.clone(),
            kind: EntryKind::Note,
        }];
        let mut inbox = Inbox::new(&received);
        worker.follow_disk_changes(&changes, &mut inbox);
        let Some(Message::Save(save)) = inbox.next() else {
            panic!("el guardado tenía que seguir esperando");
        };
        worker.record(&save);
        let id = recorded.try_recv().unwrap().unwrap().unwrap();
        assert_eq!(history.content(&b, id).unwrap(), "dos");
        assert_eq!(contents(&history, &b), ["uno", "dos"]);
        assert_eq!(history.deleted_notes().unwrap(), []);
    }

    #[test]
    fn al_renombrarse_lo_que_tiene_la_nota_tambien_es_una_version() {
        let (root, data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
        let history = Arc::new(History::new(data.path(), TEST_RETENTION));
        let worker = worker(root.path(), &history);
        let (a, b) = (
            VaultPath::new("a.md").unwrap(),
            VaultPath::new("b.md").unwrap(),
        );
        history.record(&a, "uno", SystemTime::now()).unwrap();
        // Editada y renombrada; la edición llegará después, nombrando `a.md`.
        fs::write(root.path().join("b.md"), "dos").unwrap();
        let changes = [VaultChange::Renamed {
            from: a,
            to: b.clone(),
            kind: EntryKind::Note,
        }];
        worker.follow_in_history(&changes);
        let edited: Vec<VaultPath> = changes.iter().filter_map(edited_note).cloned().collect();
        let (_messages, received) = mpsc::channel();
        worker.record_external(&edited, &mut Inbox::new(&received));
        assert_eq!(contents(&history, &b), ["uno", "dos"]);
    }

    #[test]
    fn un_guardado_que_espera_se_registra_antes_que_lo_que_ya_hay_en_disco() {
        let (root, data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
        let history = Arc::new(History::new(data.path(), TEST_RETENTION));
        let worker = worker(root.path(), &history);
        let note = VaultPath::new("a.md").unwrap();
        let (messages, received) = mpsc::channel();
        let (done, recorded) = mpsc::channel();
        // El guardado escribió "vieja", alguien escribió "nueva" encima y el
        // watcher avisó antes de que el hilo viera el mensaje del guardado.
        messages
            .send(Message::Save(Save {
                path: note.clone(),
                text: "vieja".to_owned(),
                at: SystemTime::now(),
                kind: SaveKind::Explicit,
                done,
            }))
            .unwrap();
        fs::write(root.path().join("a.md"), "nueva").unwrap();
        worker.record_external(std::slice::from_ref(&note), &mut Inbox::new(&received));

        assert!(recorded.try_recv().unwrap().unwrap().is_some());
        assert_eq!(contents(&history, &note), ["vieja", "nueva"]);
    }
}
