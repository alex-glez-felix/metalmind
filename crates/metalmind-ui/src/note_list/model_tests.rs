use std::fs::{self, File};
use std::path::Path;
use std::time::{Duration, SystemTime};

use metalmind_search::FuzzyMatcher;
use metalmind_vault::{EntryKind, Vault, VaultChange, VaultIndex};
use pretty_assertions::assert_eq;

use super::*;
use crate::note_list::cards::read_all;

/// Cada nota con su texto y hace cuántos minutos cambió.
const FILES: &[(&str, &str, u64)] = &[
    ("vieja.md", "# Una vieja\n#rust", 300),
    ("nueva.md", "La más nueva, sin tags.", 1),
    ("media.md", "#rust #archivado", 60),
    ("d/sin.md", "Otra sin tags.", 120),
];

fn write(root: &Path, name: &str, text: &str, minutes_ago: u64) {
    let path = root.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, text).unwrap();
    let at = SystemTime::now() - Duration::from_secs(minutes_ago * 60);
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(at)
        .unwrap();
}

struct Fixture {
    dir: tempfile::TempDir,
    vault: Vault,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        for (name, text, minutes) in FILES {
            write(dir.path(), name, text, *minutes);
        }
        let (vault, _updates) = Vault::open(dir.path()).unwrap();
        Self { dir, vault }
    }

    fn list(&self, source: ListSource, filter: &str) -> NoteList {
        let mut list = NoteList::new(source, filter, self.vault.snapshot(), self.vault.tags());
        let paths = list
            .source()
            .notes(&self.vault.snapshot(), &self.vault.tags());
        let step = list.set_cards(read_all(self.dir.path(), &paths));
        run(&mut list, step);
        list
    }

    /// Lo que mandaría el watcher tras cambiar `name`.
    fn update(&self, name: &str) -> VaultUpdate {
        VaultUpdate {
            changes: vec![VaultChange::Modified {
                path: VaultPath::new(name).unwrap(),
            }],
            snapshot: self.vault.snapshot(),
            links: self.vault.links(),
            tags: self.vault.tags(),
            tag_changes: Vec::new(),
        }
    }
}

/// Corre las búsquedas que pida, como el executor de fondo.
fn run(list: &mut NoteList, step: Step<Note>) {
    let mut job = step.job;
    while let Some(pending) = job {
        job = list.finish(pending.run(&mut FuzzyMatcher::paths())).job;
    }
}

fn rows(list: &NoteList) -> Vec<&str> {
    (0..list.len())
        .map(|row| list.note(row).unwrap().as_str())
        .collect()
}

#[test]
fn sin_tarjetas_no_muestra_nada_para_no_reordenar_al_llegar() {
    let fixture = Fixture::new();
    let list = NoteList::new(
        ListSource::All,
        "",
        fixture.vault.snapshot(),
        fixture.vault.tags(),
    );
    assert!(list.is_empty());
    assert!(!list.is_current());
}

#[test]
fn de_la_mas_reciente_a_la_mas_vieja() {
    let fixture = Fixture::new();
    let list = fixture.list(ListSource::All, "");
    assert_eq!(
        rows(&list),
        ["nueva.md", "media.md", "d/sin.md", "vieja.md"]
    );
    assert_eq!(list.total(), 4);
    assert_eq!(list.card(3).unwrap().title, "Una vieja");
}

#[test]
fn el_filtro_es_la_consulta_del_finder_en_su_lugar() {
    let fixture = Fixture::new();
    let mut list = fixture.list(ListSource::All, "#rust -#archivado");
    assert_eq!(rows(&list), ["vieja.md"]);
    assert_eq!(list.total(), 4);
    let step = list.edit_filter(LineEdit::DeleteToStart);
    run(&mut list, step);
    for text in ["-#", " sin"] {
        let step = list.edit_filter(LineEdit::Insert(text));
        run(&mut list, step);
    }
    assert_eq!(rows(&list), ["d/sin.md"]);
}

#[test]
fn la_fila_sigue_a_su_nota_cuando_la_lista_se_reordena() {
    let fixture = Fixture::new();
    let mut list = fixture.list(ListSource::All, "");
    list.select(2);
    assert_eq!(list.selected_note().unwrap().as_str(), "d/sin.md");
    write(fixture.dir.path(), "vieja.md", "# Una vieja\n#rust", 0);
    let (step, stale) = list.apply(&fixture.update("vieja.md"));
    run(&mut list, step);
    let step = list.set_cards(read_all(fixture.dir.path(), &stale));
    run(&mut list, step);
    assert_eq!(
        rows(&list),
        ["vieja.md", "nueva.md", "media.md", "d/sin.md"]
    );
    assert_eq!(list.selected_note().unwrap().as_str(), "d/sin.md");
    assert_eq!(list.selected_row(), Some(3));
}

#[test]
fn moverse_no_pasa_de_los_bordes() {
    let fixture = Fixture::new();
    let mut list = fixture.list(ListSource::All, "");
    list.select_previous(1);
    assert_eq!(list.selected_row(), Some(0));
    list.select_next(10);
    assert_eq!(list.selected_row(), Some(3));
    list.select_first();
    list.select_next(1);
    assert_eq!(list.selected_row(), Some(1));
    list.select_last();
    assert_eq!(list.selected_row(), Some(3));
}

#[test]
fn otra_fuente_vuelve_a_la_primera_fila() {
    let fixture = Fixture::new();
    let mut list = fixture.list(ListSource::All, "");
    list.select(3);
    let step = list.set_source(ListSource::Inbox);
    run(&mut list, step);
    assert_eq!(rows(&list), ["nueva.md", "d/sin.md"]);
    assert_eq!(list.selected_row(), Some(0));
    let step = list.set_source(ListSource::Folder("d".into()));
    run(&mut list, step);
    assert_eq!(rows(&list), ["d/sin.md"]);
}

#[test]
fn etiquetar_pide_una_nota_y_separa_los_tags() {
    let fixture = Fixture::new();
    let mut list = fixture.list(ListSource::Inbox, "");
    assert!(list.start_tagging());
    assert_eq!(list.prompt(), Some(Prompt::Tag));
    list.edit_tag_line(LineEdit::Insert("#rust, ideas  gpui"));
    let (note, tags) = list.take_tags().unwrap();
    assert_eq!(note.as_str(), "nueva.md");
    assert_eq!(tags, ["#rust", "ideas", "gpui"]);
    assert_eq!(list.prompt(), None);
    assert!(list.start_tagging());
    assert_eq!(list.take_tags(), None);

    let mut empty = fixture.list(ListSource::Tag("nada".into()), "");
    assert!(!empty.start_tagging());
}

#[test]
fn la_fila_y_la_carpeta_de_la_fuente_siguen_a_un_renombre() {
    let fixture = Fixture::new();
    let renamed_index = VaultIndex::from_entries(
        [
            ("vieja.md", EntryKind::Note),
            ("nueva.md", EntryKind::Note),
            ("media.md", EntryKind::Note),
            ("e", EntryKind::Folder),
            ("e/sin.md", EntryKind::Note),
        ]
        .map(|(name, kind)| (VaultPath::new(name).unwrap(), kind)),
    );
    let update = VaultUpdate {
        changes: vec![VaultChange::Renamed {
            from: VaultPath::new("d").unwrap(),
            to: VaultPath::new("e").unwrap(),
            kind: EntryKind::Folder,
        }],
        snapshot: Arc::new(renamed_index),
        ..fixture.update("nueva.md")
    };
    for source in [ListSource::All, ListSource::Folder("d".into())] {
        let mut list = fixture.list(source, "");
        assert!(list.reveal(&VaultPath::new("d/sin.md").unwrap()).is_some());
        let (step, _unread) = list.apply(&update);
        run(&mut list, step);
        assert_eq!(list.selected_note().unwrap().as_str(), "e/sin.md");
        assert_eq!(
            list.card(list.selected_row().unwrap()).unwrap().title,
            "sin"
        );
    }
    let mut folder = fixture.list(ListSource::Folder("d".into()), "");
    let (step, _unread) = folder.apply(&update);
    run(&mut folder, step);
    assert_eq!(folder.source(), &ListSource::Folder("e".into()));
    assert_eq!(rows(&folder), ["e/sin.md"]);
}

#[test]
fn un_lote_antes_de_las_tarjetas_no_muestra_la_lista_sin_orden() {
    let fixture = Fixture::new();
    let mut list = NoteList::new(
        ListSource::All,
        "",
        fixture.vault.snapshot(),
        fixture.vault.tags(),
    );
    let (step, _unread) = list.apply(&fixture.update("nueva.md"));
    run(&mut list, step);
    assert!(list.is_empty());
    assert!(!list.is_current());
    let paths = list
        .source()
        .notes(&fixture.vault.snapshot(), &fixture.vault.tags());
    let step = list.set_cards(read_all(fixture.dir.path(), &paths));
    run(&mut list, step);
    assert_eq!(
        rows(&list),
        ["nueva.md", "media.md", "d/sin.md", "vieja.md"]
    );
    assert_eq!(list.selected_note().unwrap().as_str(), "nueva.md");
}

#[test]
fn la_fuente_sigue_a_su_tag_renombrado() {
    let fixture = Fixture::new();
    let mut list = fixture.list(ListSource::Tag("rust".into()), "");
    let step = list.rename_tag("#rust", "#lenguaje");
    run(&mut list, step);
    assert_eq!(list.source(), &ListSource::Tag("lenguaje".into()));
}
