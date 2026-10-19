use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};

use metalmind_vault::{EntryKind, VaultChange, VaultPath};
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::*;
use crate::workspace::test_session::Session;

const SECOND: Duration = Duration::from_secs(1);

/// Un reloj que solo avanza cuando el test lo dice.
#[derive(Debug)]
struct TestClock(Cell<Instant>);

impl Clock for TestClock {
    fn now(&self) -> Instant {
        self.0.get()
    }
}

/// Una nota en disco abierta con autoguardado de un segundo.
struct Autosaving {
    dir: TempDir,
    clock: Rc<TestClock>,
    session: Session,
}

impl Autosaving {
    fn new(notes: &[(&str, &str)]) -> Self {
        let dir = TempDir::new().unwrap();
        for (name, text) in notes {
            fs::write(dir.path().join(name), text).unwrap();
        }
        let session = Session::open(&dir.path().join(notes[0].0));
        Self::with(dir, session)
    }

    fn with(dir: TempDir, mut session: Session) -> Self {
        let clock = Rc::new(TestClock(Cell::new(Instant::now())));
        let shared: Rc<dyn Clock> = Rc::clone(&clock) as Rc<dyn Clock>;
        session
            .workspace
            .set_autosave(Some(Autosave::new(SECOND, shared)));
        Self {
            dir,
            clock,
            session,
        }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    fn read(&self, name: &str) -> String {
        fs::read_to_string(self.path(name)).unwrap()
    }

    fn wait(&self, time: Duration) {
        self.clock.0.set(self.clock.0.get() + time);
    }

    fn tick(&mut self) -> bool {
        self.session.workspace.autosave_if_due()
    }

    fn state(&self) -> Option<SaveState> {
        self.session.view(self.session.pane()).save
    }

    fn message(&self) -> Option<&Message> {
        self.session.workspace.message()
    }

    /// El watcher avisa que `name` cambió en disco.
    fn modified_on_disk(&mut self, name: &str) {
        let change = VaultChange::Modified {
            path: VaultPath::new(name).unwrap(),
        };
        let root = self.dir.path().to_owned();
        self.session.workspace.files_changed(&root, &[change]);
    }
}

fn edit_file(session: &mut Session, path: &Path) {
    session.keys(&format!(":e {}⏎", path.display()));
}

#[test]
fn guarda_tras_un_segundo_sin_editar() {
    let mut autosaving = Autosaving::new(&[("a.md", "uno\n")]);
    autosaving.session.keys("Ax⎋");
    autosaving.wait(SECOND / 2);
    assert!(!autosaving.tick());
    autosaving.session.keys("Ay⎋");
    autosaving.wait(SECOND * 8 / 10);
    assert!(!autosaving.tick(), "cada edición espera un segundo entero");
    assert_eq!(autosaving.read("a.md"), "uno\n");
    autosaving.wait(SECOND / 5);
    let before = autosaving.message().cloned();
    assert!(autosaving.tick());
    assert_eq!(autosaving.read("a.md"), "unoxy\n");
    assert!(!autosaving.session.is_modified());
    assert_eq!(autosaving.session.workspace.autosave_deadline(), None);
    assert_eq!(autosaving.message().cloned(), before, "sin avisar nada");
}

#[test]
fn moverse_no_es_editar() {
    let mut autosaving = Autosaving::new(&[("a.md", "uno\ndos\n")]);
    autosaving.session.keys("jk");
    assert_eq!(autosaving.session.workspace.autosave_deadline(), None);
}

#[test]
fn guarda_a_mitad_de_un_insert_y_al_salir_queda_limpio() {
    let mut autosaving = Autosaving::new(&[("a.md", "uno\n")]);
    autosaving.session.keys("ihola ");
    autosaving.wait(SECOND);
    assert!(autosaving.tick());
    assert_eq!(autosaving.read("a.md"), "hola uno\n");
    assert_eq!(autosaving.state(), Some(SaveState::Saved));
    assert!(!autosaving.session.view(autosaving.session.pane()).unsaved());
    assert!(autosaving.session.workspace.may_close_window());
    assert!(
        !autosaving.session.workspace.autosave_all(),
        "nada que reescribir"
    );
    autosaving.session.keys("⎋");
    assert!(!autosaving.session.is_modified());
    assert_eq!(autosaving.session.workspace.autosave_deadline(), None);
    autosaving.session.keys("u");
    assert!(autosaving.session.is_modified());
}

#[test]
fn cambiar_de_nota_en_el_panel_guarda_la_que_deja() {
    let mut autosaving = Autosaving::new(&[("a.md", "uno\n"), ("b.md", "dos\n")]);
    autosaving.session.keys("Ax⎋");
    let b = autosaving.path("b.md");
    edit_file(&mut autosaving.session, &b);
    assert_eq!(autosaving.read("a.md"), "unox\n", "sin esperar el segundo");
    assert_eq!(autosaving.session.file(), "b.md");
    autosaving.session.keys("Ay⎋⌃o");
    assert_eq!(
        autosaving.read("b.md"),
        "dosy\n",
        "también al volver con ctrl-o"
    );
    assert_eq!(autosaving.session.file(), "a.md");
}

#[test]
fn un_buffer_sin_archivo_no_se_autoguarda() {
    let dir = TempDir::new().unwrap();
    let mut autosaving = Autosaving::with(dir, Session::unnamed());
    autosaving.session.keys("ihola⎋");
    autosaving.wait(SECOND);
    assert!(!autosaving.tick());
    assert!(autosaving.session.is_modified());
    assert_eq!(autosaving.state(), Some(SaveState::Unsaved));
    assert!(!autosaving.session.workspace.may_close_window());
}

#[test]
fn sin_autoguardado_la_salida_se_protege_como_en_vim() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.md"), "uno\n").unwrap();
    let mut session = Session::open(&dir.path().join("a.md"));
    session.keys("Ax⎋");
    assert!(!session.workspace.may_close_window());
    assert_eq!(session.view(session.pane()).save, None);
    assert_eq!(
        fs::read_to_string(dir.path().join("a.md")).unwrap(),
        "uno\n"
    );
}

#[test]
fn con_autoguardado_cerrar_guarda_antes() {
    let mut autosaving = Autosaving::new(&[("a.md", "uno\n"), ("b.md", "dos\n")]);
    autosaving.session.keys("Ax⎋");
    let b = autosaving.path("b.md");
    autosaving.session.keys("⌃wv");
    edit_file(&mut autosaving.session, &b);
    autosaving.session.keys("Ay⎋");
    assert!(autosaving.session.workspace.may_close_window());
    assert_eq!(autosaving.read("a.md"), "unox\n");
    assert_eq!(autosaving.read("b.md"), "dosy\n");
}

#[test]
fn perder_el_foco_guarda_todo_sin_esperar() {
    let mut autosaving = Autosaving::new(&[("a.md", "uno\n")]);
    autosaving.session.keys("Ax⎋");
    assert!(autosaving.session.workspace.autosave_all());
    assert_eq!(autosaving.read("a.md"), "unox\n");
    assert!(
        !autosaving.session.workspace.autosave_all(),
        "ya no hay nada"
    );
}

#[test]
fn no_pisa_lo_que_cambio_en_disco() {
    let mut autosaving = Autosaving::new(&[("a.md", "uno\n")]);
    autosaving.session.keys("Ax⎋");
    fs::write(autosaving.path("a.md"), "de fuera\n").unwrap();
    autosaving.wait(SECOND);
    assert!(autosaving.tick());
    assert_eq!(autosaving.read("a.md"), "de fuera\n");
    let w12 = format!(
        "W12: Warning: File \"{}\" has changed and the buffer was changed in Vim as well",
        autosaving.path("a.md").display()
    );
    assert_eq!(autosaving.message(), Some(&Message::Error(w12)));
    assert_eq!(autosaving.state(), Some(SaveState::ChangedOnDisk));

    // Ni el autoguardado ni `:w` lo pisan; `:w!` sí.
    autosaving.session.keys("Ay⎋");
    autosaving.wait(SECOND);
    assert!(!autosaving.tick());
    autosaving.session.keys(":w⏎");
    assert_eq!(
        autosaving.message(),
        Some(&Message::Error(
            "WARNING: The file has been changed since reading it!!!".to_owned()
        ))
    );
    assert_eq!(autosaving.read("a.md"), "de fuera\n");
    autosaving.session.keys(":w!⏎");
    assert_eq!(autosaving.read("a.md"), "unoxy\n");
    assert_eq!(autosaving.state(), Some(SaveState::Saved));
    autosaving.session.keys("Az⎋");
    autosaving.wait(SECOND);
    assert!(autosaving.tick());
    assert_eq!(autosaving.read("a.md"), "unoxyz\n");
}

#[test]
fn el_watcher_avisa_antes_del_autoguardado_y_e_bang_trae_lo_de_disco() {
    let mut autosaving = Autosaving::new(&[("a.md", "uno\n")]);
    autosaving.session.keys("Ax⎋");
    fs::write(autosaving.path("a.md"), "de fuera\n").unwrap();
    autosaving.modified_on_disk("a.md");
    assert!(matches!(autosaving.message(), Some(Message::Error(text)) if text.starts_with("W12")));
    autosaving.wait(SECOND);
    assert!(
        !autosaving.tick(),
        "ya estaba anotado: no hay nada que hacer"
    );
    autosaving.session.keys(":e!⏎");
    assert_eq!(autosaving.session.text(), "de fuera");
    assert_eq!(autosaving.state(), Some(SaveState::Saved));
    autosaving.session.keys("Ax⎋");
    autosaving.wait(SECOND);
    assert!(autosaving.tick());
    assert_eq!(autosaving.read("a.md"), "de fuerax\n");
}

#[test]
fn el_eco_del_propio_guardado_no_es_un_cambio_de_fuera() {
    let mut autosaving = Autosaving::new(&[("a.md", "uno\n")]);
    autosaving.session.keys("Ax⎋");
    autosaving.wait(SECOND);
    assert!(autosaving.tick());
    autosaving.session.keys("Ay⎋");
    autosaving.modified_on_disk("a.md");
    autosaving.wait(SECOND);
    assert!(autosaving.tick());
    assert_eq!(autosaving.read("a.md"), "unoxy\n");
}

#[test]
fn un_archivo_borrado_fuera_no_se_vuelve_a_crear() {
    let mut autosaving = Autosaving::new(&[("a.md", "uno\n")]);
    autosaving.session.keys("Ax⎋");
    fs::remove_file(autosaving.path("a.md")).unwrap();
    let removed = VaultChange::Removed {
        path: VaultPath::new("a.md").unwrap(),
        kind: EntryKind::Note,
    };
    let root = autosaving.dir.path().to_owned();
    autosaving
        .session
        .workspace
        .files_changed(&root, &[removed]);
    autosaving.wait(SECOND);
    assert!(!autosaving.tick());
    assert!(!autosaving.path("a.md").exists());
}

#[test]
fn el_indicador_no_parpadea_mientras_se_escribe() {
    let mut autosaving = Autosaving::new(&[("a.md", "uno\n")]);
    assert_eq!(autosaving.state(), Some(SaveState::Saved));
    // Una tecla cada medio segundo: el autoguardado nunca llega.
    for _ in 0..10 {
        autosaving.session.keys("ix⎋");
        assert_eq!(autosaving.state(), Some(SaveState::Saved));
        autosaving.wait(SECOND / 2);
    }
    autosaving.session.keys("ix⎋");
    assert_eq!(
        autosaving.state(),
        Some(SaveState::Unsaved),
        "cinco segundos con cambios sin guardar"
    );
    autosaving.wait(SECOND);
    assert!(autosaving.tick());
    assert_eq!(autosaving.state(), Some(SaveState::Saved));
    autosaving.session.keys("ix⎋");
    assert_eq!(
        autosaving.state(),
        Some(SaveState::Saved),
        "vuelve a contar"
    );
}

#[test]
fn si_escribir_falla_se_reintenta_sin_otra_edicion() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("falta").join("a.md");
    let session = Session::open(&path);
    let mut autosaving = Autosaving::with(dir, session);
    autosaving.session.keys("ihola⎋");
    autosaving.wait(SECOND);
    assert!(autosaving.tick());
    assert!(matches!(autosaving.message(), Some(Message::Error(_))));
    assert!(autosaving.session.is_modified());
    fs::create_dir(autosaving.path("falta")).unwrap();
    autosaving.wait(SECOND);
    assert!(!autosaving.tick(), "espera antes de reintentar");
    autosaving.wait(SECOND * 4);
    assert!(autosaving.tick());
    assert_eq!(autosaving.read("falta/a.md"), "hola\n");
    assert_eq!(autosaving.session.workspace.autosave_deadline(), None);
}
