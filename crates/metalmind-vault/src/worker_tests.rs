use std::fs;
use std::time::Duration;

use notify::EventKind;
use notify::event::{ModifyKind, RenameMode};
use tempfile::TempDir;

use super::*;
use crate::debounce::{Batch, Debouncer};
use crate::history::TEST_RETENTION;
use crate::scan::scan;
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
    let debouncer = Debouncer::new(Duration::from_millis(10), |_| {}).unwrap();
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

/// Un guardado que ya escribió `text` en `path` y cuya versión espera.
fn queued_save(
    path: &VaultPath,
    text: &str,
) -> (
    Message,
    mpsc::Receiver<Result<Option<VersionId>, HistoryError>>,
) {
    let (done, recorded) = mpsc::channel();
    let save = Save {
        path: path.clone(),
        text: text.to_owned(),
        at: SystemTime::now(),
        kind: SaveKind::Explicit,
        done,
    };
    (Message::Save(save), recorded)
}

/// El lote del watcher con el renombre de `from` a `to`.
fn renamed(root: &Path, from: &VaultPath, to: &VaultPath) -> Message {
    let event = notify::Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Both)))
        .add_path(from.to_fs(root))
        .add_path(to.to_fs(root));
    Message::Disk(Batch {
        events: vec![event],
        errors: Vec::new(),
    })
}

#[test]
fn guardados_en_cola_detras_de_un_renombre_quedan_en_la_ruta_nueva() {
    let (root, data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let history = Arc::new(History::new(data.path(), TEST_RETENTION));
    let (a, b) = (
        VaultPath::new("a.md").unwrap(),
        VaultPath::new("b.md").unwrap(),
    );
    fs::write(root.path().join("a.md"), "uno").unwrap();
    history.record(&a, "uno", SystemTime::now()).unwrap();
    let worker = worker(root.path(), &history);
    worker.index.store(Arc::new(scan(root.path(), None)));
    // "dos" se guardó en `a.md`, después se renombró a `b.md` y ahí se
    // guardó "tres": los mensajes llegan en ese orden.
    fs::write(root.path().join("a.md"), "dos").unwrap();
    fs::rename(root.path().join("a.md"), root.path().join("b.md")).unwrap();
    fs::write(root.path().join("b.md"), "tres").unwrap();
    let (before, before_recorded) = queued_save(&a, "dos");
    let (after, after_recorded) = queued_save(&b, "tres");
    let (messages, received) = mpsc::channel();
    for message in [before, renamed(root.path(), &a, &b), after, Message::Stop] {
        messages.send(message).unwrap();
    }

    worker.run(&received);

    assert!(before_recorded.try_recv().unwrap().unwrap().is_some());
    let after = after_recorded.try_recv().unwrap().unwrap().unwrap();
    assert_eq!(contents(&history, &b), ["uno", "dos", "tres"]);
    assert_eq!(history.content(&b, after).unwrap(), "tres");
    assert_eq!(history.deleted_notes().unwrap(), []);
}
