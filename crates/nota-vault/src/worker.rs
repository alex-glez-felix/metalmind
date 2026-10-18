//! El hilo de un vault abierto: mantiene el índice al día con lo que ve el
//! watcher y es el único que escribe el historial, así que las versiones y
//! los renombres le llegan en el orden en que pasaron.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError, mpsc};
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
use crate::vault::VaultUpdate;
use crate::watch::{touched, touched_by_errors};

pub(crate) enum Message {
    Disk(DebounceEventResult),
    Save(Save),
    Stop,
}

/// Una nota que ya quedó en disco y cuya versión falta registrar.
pub(crate) struct Save {
    pub path: VaultPath,
    pub text: String,
    pub at: SystemTime,
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
}

impl Worker {
    pub(crate) fn run(self, messages: &mpsc::Receiver<Message>) {
        self.prune_history();
        let mut indexes = Indexes {
            files: VaultIndex::clone(&self.index.load()),
            links: LinkIndex::clone(&self.links.load()),
            tags: Tags::clone(&self.tags.load()),
        };
        let mut inbox = Inbox::new(messages);
        while let Some(message) = inbox.next() {
            match message {
                Message::Disk(first) => self.on_disk(&mut indexes, first, &mut inbox),
                Message::Save(save) => self.record(&save),
                Message::Stop => return,
            }
        }
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
        let changes = refresh(files, &self.root, &touched);
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
        if changes.is_empty() && !reread && tag_changes.is_empty() {
            return;
        }
        // Antes de publicar, para que quien reciba el cambio ya vea el
        // historial en la ruta nueva.
        self.follow_in_history(&changes);
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

    /// Solo llegan guardados si hay historial.
    fn record(&self, save: &Save) {
        let Some(history) = &self.history else {
            return;
        };
        let result = history.record(&save.path, &save.text, save.at);
        if let Err(error) = &result {
            log::warn!("{error}");
        }
        // Quien guardó puede no esperar el resultado.
        let _ = save.done.send(result);
    }

    /// Lo que cambió fuera de nota también es una versión (06 §3). Un
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
            (inbox.pending_saves(), self.read_notes(notes))
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

    /// La retención se aplica al abrir el vault, aquí y no en `open`: podar
    /// lee cada log y no tiene por qué demorar la apertura. Lo que llegue
    /// mientras tanto espera en el canal.
    fn prune_history(&self) {
        let Some(history) = &self.history else {
            return;
        };
        if let Err(error) = history.prune(SystemTime::now()) {
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
    fn pending_saves(&mut self) -> Vec<Save> {
        self.queue.extend(self.receiver.try_iter());
        let mut saves = Vec::new();
        let mut rest = VecDeque::with_capacity(self.queue.len());
        for message in self.queue.drain(..) {
            match message {
                Message::Save(save) => saves.push(save),
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

    use nota_config::HistoryRetention;
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn un_guardado_que_espera_se_registra_antes_que_lo_que_ya_hay_en_disco() {
        let (root, data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
        let history = Arc::new(History::new(data.path(), HistoryRetention::default()));
        let (updates, _published) = mpsc::channel();
        let worker = Worker {
            root: root.path().to_owned(),
            index: Arc::new(ArcSwap::from_pointee(VaultIndex::default())),
            links: Arc::new(ArcSwap::from_pointee(LinkIndex::default())),
            tags: Arc::new(ArcSwap::from_pointee(Tags::build(
                VaultPath::fixed(crate::tags::DEFAULT_TAGS_FOLDER),
                &VaultIndex::default(),
                |_: &VaultPath| None,
            ))),
            history: Some(Arc::clone(&history)),
            saving: Arc::new(Mutex::new(())),
            updates,
            stop: Arc::new(AtomicBool::new(false)),
        };
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
                done,
            }))
            .unwrap();
        fs::write(root.path().join("a.md"), "nueva").unwrap();
        worker.record_external(std::slice::from_ref(&note), &mut Inbox::new(&received));

        assert!(recorded.try_recv().unwrap().unwrap().is_some());
        let contents: Vec<String> = history
            .versions(&note)
            .unwrap()
            .iter()
            .map(|version| history.content(&note, version.id).unwrap())
            .collect();
        assert_eq!(contents, ["vieja", "nueva"]);
    }
}
