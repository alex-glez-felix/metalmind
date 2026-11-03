use std::collections::BTreeMap;
use std::fs;
use std::time::Duration;

use notify::EventKind;
use notify::event::{ModifyKind, RenameMode};
use tempfile::TempDir;

use super::*;
use crate::debounce::{Batch, Debouncer};
use crate::history::TEST_RETENTION;
use crate::note_scan::index_notes;
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
        properties: Arc::default(),
        history: Some(Arc::clone(history)),
        cache_refresh: None,
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
    worker.follow_disk_changes(&changes, Vec::new(), &mut Inbox::new(&received));
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
    worker.follow_disk_changes(&changes, Vec::new(), &mut inbox);
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
    read_and_record(&worker, &edited, &mut Inbox::new(&received));
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
    read_and_record(
        &worker,
        std::slice::from_ref(&note),
        &mut Inbox::new(&received),
    );

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

/// Lo que hace un lote con `edited` en el historial, sin lo demás del lote.
fn read_and_record(worker: &Worker, edited: &[VaultPath], inbox: &mut Inbox) {
    let read = |path: &VaultPath| read_note(&worker.root, path);
    let mut notes = NoteBatch::new(&read);
    for save in worker.read_external(edited, &mut notes, inbox) {
        worker.record(&save);
    }
    worker.record_external(notes.into_texts(edited));
}

/// Un guardado que llega después de leer el disco no puede registrarse antes
/// que lo leído: la versión vieja quedaría como la última.
#[test]
fn lo_guardado_despues_de_leer_va_detras_de_lo_leido() {
    let (root, data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let history = Arc::new(History::new(data.path(), TEST_RETENTION));
    let worker = worker(root.path(), &history);
    let note = VaultPath::new("a.md").unwrap();
    fs::write(root.path().join("a.md"), "de fuera").unwrap();
    let read = |path: &VaultPath| read_note(&worker.root, path);
    let mut notes = NoteBatch::new(&read);
    let (messages, received) = mpsc::channel();
    let mut inbox = Inbox::new(&received);
    let external = std::slice::from_ref(&note);
    let saves = worker.read_external(external, &mut notes, &mut inbox);
    assert!(saves.is_empty());

    let (save, _recorded) = queued_save(&note, "guardada");
    messages.send(save).unwrap();
    let saves = worker.follow_disk_changes(&[], saves, &mut inbox);
    assert!(saves.is_empty(), "no toca a un renombre: espera su turno");
    worker.record_external(notes.into_texts(external));
    let Some(Message::Save(save)) = inbox.next() else {
        panic!("el guardado tenía que seguir esperando");
    };
    worker.record(&save);
    assert_eq!(contents(&history, &note), ["de fuera", "guardada"]);
}

const TAGS_FOLDER: &str = crate::tags::DEFAULT_TAGS_FOLDER;

fn full_indexes(root: &Path) -> Indexes {
    let read = |path: &VaultPath| read_note(root, path);
    let files = scan(root, None);
    let indexes = index_notes(root, &files, &VaultPath::fixed(TAGS_FOLDER), None, &read);
    Indexes {
        files,
        links: indexes.links,
        tags: indexes.tags,
        properties: indexes.properties,
    }
}

/// Cada nota que el lote mira se lee una vez: los links, los tags y el
/// historial ven el mismo contenido. Una renombrada sin cambios la lee solo el
/// historial, y lo que no tocó el lote no se lee.
#[test]
fn un_lote_lee_cada_nota_una_sola_vez() {
    let (root, data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let root = root.path().canonicalize().unwrap();
    let history = Arc::new(History::new(data.path(), TEST_RETENTION));
    let write = |name: &str, text: &str| fs::write(root.join(name), text).unwrap();
    fs::create_dir(root.join("tags")).unwrap();
    write("editada.md", "[[nueva]] #uno");
    write("movida.md", "---\ntítulo: Sigue\n---\n[[editada]] #dos");
    write("quieta.md", "[[editada]] #tres");
    write("tags/uno.md", "");
    let mut indexes = full_indexes(&root);
    let worker = worker(&root, &history);
    worker.index.store(Arc::new(indexes.files.clone()));

    write("editada.md", "[[nueva]] [[movida-2]] #otro");
    write("nueva.md", "#cuatro");
    write("tags/uno.md", "---\ncolor: naranja\n---\n#cinco");
    fs::rename(root.join("movida.md"), root.join("movida-2.md")).unwrap();
    let vault_path = |name: &str| VaultPath::new(name).unwrap();
    let written = ["editada.md", "nueva.md", "tags/uno.md"].map(vault_path);
    let mut touched = Touched {
        renames: vec![(vault_path("movida.md"), vault_path("movida-2.md"))],
        written: written.iter().cloned().collect(),
        // Un chmod sobre lo mismo no la vuelve a leer.
        reread: BTreeSet::from([vault_path("editada.md")]),
        ..Touched::default()
    };
    touched.paths.extend(written);
    touched
        .paths
        .extend(["movida.md", "movida-2.md"].map(vault_path));

    let reads = Mutex::new(BTreeMap::<String, usize>::new());
    let read = |path: &VaultPath| {
        let mut reads = reads.lock().unwrap();
        *reads.entry(path.to_string()).or_default() += 1;
        read_note(&root, path)
    };
    let (_messages, received) = mpsc::channel();
    worker.on_touched(&mut indexes, &touched, &read, &mut Inbox::new(&received));

    let once: BTreeMap<String, usize> = ["editada.md", "movida-2.md", "nueva.md", "tags/uno.md"]
        .map(|name| (name.to_owned(), 1))
        .into();
    assert_eq!(reads.into_inner().unwrap(), once);
    let full = full_indexes(&root);
    assert_eq!(indexes.links, full.links);
    assert_eq!(indexes.tags, full.tags);
    assert_eq!(indexes.properties, full.properties);
    assert_eq!(
        contents(&history, &vault_path("editada.md")),
        ["[[nueva]] [[movida-2]] #otro"]
    );
    assert_eq!(*worker.links.load_full(), full.links);
}

/// Sin historial, nadie necesita el texto de una nota que solo se renombró.
#[test]
fn sin_historial_un_renombre_no_lee_la_nota() {
    let (root, data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let root = root.path().canonicalize().unwrap();
    let history = Arc::new(History::new(data.path(), TEST_RETENTION));
    fs::write(root.join("a.md"), "[[b]] #uno").unwrap();
    let mut indexes = full_indexes(&root);
    let mut worker = worker(&root, &history);
    worker.history = None;
    worker.index.store(Arc::new(indexes.files.clone()));
    fs::rename(root.join("a.md"), root.join("b.md")).unwrap();
    let (a, b) = (
        VaultPath::new("a.md").unwrap(),
        VaultPath::new("b.md").unwrap(),
    );
    let touched = Touched {
        paths: BTreeSet::from([a.clone(), b.clone()]),
        renames: vec![(a, b)],
        ..Touched::default()
    };
    let read = |path: &VaultPath| panic!("no tenía que leer {path}");
    let (_messages, received) = mpsc::channel();
    worker.on_touched(&mut indexes, &touched, &read, &mut Inbox::new(&received));
    assert_eq!(indexes.links, full_indexes(&root).links);
}

/// Un chmod que solo cambia el frontmatter también se publica: sin eso nadie
/// vería las propiedades hasta el próximo cambio de la nota.
#[test]
fn un_chmod_que_solo_cambia_propiedades_se_publica() {
    let (root, data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let root = root.path().canonicalize().unwrap();
    let history = Arc::new(History::new(data.path(), TEST_RETENTION));
    fs::write(root.join("a.md"), "---\ntítulo: Uno\n---\n").unwrap();
    let mut indexes = full_indexes(&root);
    let worker = worker(&root, &history);
    worker.index.store(Arc::new(indexes.files.clone()));
    fs::write(root.join("a.md"), "---\ntítulo: Dos\n---\n").unwrap();
    let note = VaultPath::new("a.md").unwrap();
    let touched = Touched {
        reread: BTreeSet::from([note]),
        ..Touched::default()
    };
    let read = |path: &VaultPath| read_note(&root, path);
    let (_messages, received) = mpsc::channel();
    worker.on_touched(&mut indexes, &touched, &read, &mut Inbox::new(&received));
    assert_eq!(indexes.properties, full_indexes(&root).properties);
    assert_eq!(*worker.properties.load_full(), indexes.properties);
}
