//! El flujo entero sin ventana: lo que se pide, lo que queda en disco y los
//! eventos que recibiría la ventana.

use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, mpsc};

use gpui_kit::{AppContext as _, Entity, Subscription, TestAppContext};
use metalmind_config::Config;
use metalmind_keymap::{Command, Invocation, Keymap};
use metalmind_vault::{EntryKind, FileEdits, LinkIndex, TextEdit, Vault, VaultPath, VaultUpdate};
use metalmind_vim::Markdown;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::{VaultChanges, VaultChangesEvent};
use crate::change_preview::{Blocked, ChangePreviewEvent, ChangePreviewView};
use crate::clipboard::SystemClipboard;
use crate::document::Document;
use crate::finder::{Prepared, VaultSession};
use crate::note_list::{ListSource, NoteListView};
use crate::session::VaultState;
use crate::tag_change::TagRequest;
use crate::theme::Theme;
use crate::workspace::{LineNumbers, LinePosition, Options, PaneOptions, Workspace};

/// Un evento como se puede comparar; la vista de `ShowPreview` queda en
/// [`Fixture::shown`], que hace de la ventana y la mantiene viva.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Seen {
    Show,
    Close { focus_sidebar: bool },
    OpenLine { path: String, at: LinePosition },
    Planned,
    Renamed(String),
    SavedViews,
}

struct Fixture<'a> {
    cx: &'a mut TestAppContext,
    dir: TempDir,
    root: PathBuf,
    changes: Entity<VaultChanges>,
    workspace: Entity<Workspace>,
    list: Entity<NoteListView>,
    seen: Rc<RefCell<Vec<Seen>>>,
    shown: Rc<RefCell<Option<Entity<ChangePreviewView>>>>,
    _events: Subscription,
    /// Sin leer: los lotes del watcher no llegan a la prueba.
    _watcher: mpsc::Receiver<VaultUpdate>,
}

fn path(path: &str) -> VaultPath {
    VaultPath::new(path).unwrap()
}

/// Un vault con `notes` y `opened` en el único panel.
fn fixture<'a>(cx: &'a mut TestAppContext, notes: &[(&str, &str)], opened: &str) -> Fixture<'a> {
    let dir = tempfile::tempdir().unwrap();
    for (name, text) in notes {
        let file = dir.path().join(name);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, text).unwrap();
    }
    let (vault, watcher) = Vault::open(dir.path()).unwrap();
    let root = vault.root().to_owned();
    let document = root.join(opened);
    let prepared = Prepared::without_updates(vault);
    cx.update(|cx| {
        cx.set_global(Theme::with_font(
            &Config::defaults().unwrap(),
            "Mono".into(),
        ));
    });
    let session = cx.update(|cx| cx.new(|cx| VaultSession::new(prepared, cx)));
    let workspace = cx.update(|cx| {
        cx.new(|_| {
            let pane = PaneOptions {
                numbers: LineNumbers::HYBRID,
                wrap: true,
            };
            let options = Options {
                scrolloff: 0,
                tabstop: 8,
                expandtab: false,
                gutter_marks: false,
                markdown: Markdown::default(),
            };
            let document = Document::open(&document).unwrap();
            Workspace::new(document, pane, options, &Keymap::default())
        })
    });
    let list = cx.update(|cx| {
        let state = VaultState::default();
        cx.new(|cx| NoteListView::new(&session, &state.list, workspace.clone(), cx))
    });
    let changes = cx.update(|cx| {
        cx.new(|cx| VaultChanges::new(workspace.clone(), session.clone(), list.clone(), cx))
    });
    let seen = Rc::new(RefCell::new(Vec::new()));
    let shown = Rc::new(RefCell::new(None));
    let events = cx.update(|cx| {
        let (seen, shown) = (Rc::clone(&seen), Rc::clone(&shown));
        cx.subscribe(&changes, move |_, event, _| {
            seen.borrow_mut().push(match event {
                VaultChangesEvent::ShowPreview(view) => {
                    *shown.borrow_mut() = Some(view.clone());
                    Seen::Show
                }
                VaultChangesEvent::ClosePreview { focus_sidebar } => Seen::Close {
                    focus_sidebar: *focus_sidebar,
                },
                VaultChangesEvent::OpenLine { path, at } => Seen::OpenLine {
                    path: path.to_string(),
                    at: *at,
                },
                VaultChangesEvent::RenamePlanned => Seen::Planned,
                VaultChangesEvent::Renamed(to) => Seen::Renamed(to.to_string()),
                VaultChangesEvent::SavedViewsChanged => Seen::SavedViews,
            });
        })
    });
    Fixture {
        cx,
        dir,
        root,
        changes,
        workspace,
        list,
        seen,
        shown,
        _events: events,
        _watcher: watcher,
    }
}

fn rename_tag(from: &str, to: &str) -> TagRequest {
    TagRequest::Rename {
        from: from.to_owned(),
        to: to.to_owned(),
    }
}

impl Fixture<'_> {
    fn read(&self, name: &str) -> String {
        fs::read_to_string(self.dir.path().join(name)).unwrap()
    }

    fn exists(&self, name: &str) -> bool {
        self.dir.path().join(name).exists()
    }

    /// Lo que llegó desde la última vez.
    fn seen(&mut self) -> Vec<Seen> {
        self.seen.borrow_mut().drain(..).collect()
    }

    fn view(&self) -> Entity<ChangePreviewView> {
        self.shown.borrow().clone().unwrap()
    }

    fn change_tags(&mut self, request: &TagRequest) {
        self.changes
            .update(self.cx, |changes, cx| changes.change_tags(request, cx));
    }

    fn undo(&mut self) {
        self.changes.update(self.cx, VaultChanges::undo);
    }

    /// Pide el renombre y espera su plan.
    fn plan_rename(&mut self, from: &str, to: &str) {
        self.changes.update(self.cx, |changes, cx| {
            changes.rename(&path(from), &path(to), cx);
        });
        self.cx.run_until_parked();
        assert_eq!(self.seen(), [Seen::Planned]);
    }

    fn finish_rename(&mut self, overlay_open: bool) {
        self.changes.update(self.cx, |changes, cx| {
            changes.finish_rename(overlay_open, cx);
        });
    }

    /// Lo que la vista previa le diría a quien la muestra.
    fn press(&mut self, event: ChangePreviewEvent) {
        self.view().update(self.cx, |_, cx| cx.emit(event));
    }

    /// El último aviso que se ve.
    fn toast(&mut self) -> String {
        self.workspace.read_with(self.cx, |workspace, _| {
            let toasts = workspace.messages().toasts();
            let (_, message) = toasts.last().unwrap();
            message.text().to_owned()
        })
    }

    fn toasts(&mut self) -> Vec<String> {
        self.workspace.read_with(self.cx, |workspace, _| {
            workspace
                .messages()
                .toasts()
                .iter()
                .map(|(_, message)| message.text().to_owned())
                .collect()
        })
    }

    /// Le mete a `name`, abierto, un cambio sin guardar al principio.
    fn dirty(&mut self, name: &str) {
        let file = self.root.join(name);
        let edits = FileEdits {
            path: path(name),
            edits: vec![TextEdit {
                range: 0..0,
                old: String::new(),
                new: "x\n".to_owned(),
                line: 0,
            }],
            base_len: None,
        };
        self.workspace.update(self.cx, |workspace, cx| {
            cx.notify();
            workspace.edit_file(&file, &edits).unwrap();
        });
    }

    fn buffer(&mut self, name: &str) -> String {
        let file = self.root.join(name);
        self.workspace
            .read_with(self.cx, |workspace, _| workspace.file_text(&file).unwrap())
    }
}

#[path = "board_tests.rs"]
mod board_tests;

/// Otro índice con el mismo contenido que `notes`, como el de un lote nuevo.
fn another_index(notes: &[(&str, &str)]) -> Arc<LinkIndex> {
    Arc::new(LinkIndex::from_texts(
        notes
            .iter()
            .map(|&(name, text)| (path(name), EntryKind::Note, text)),
    ))
}

/// Pedir muestra la vista previa sin tocar nada; aplicar la cierra, escribe y
/// deja qué deshacer, con su aviso; deshacer lo devuelve.
#[gpui_kit::test]
fn un_cambio_de_tags_se_muestra_se_aplica_y_se_deshace(cx: &mut TestAppContext) {
    let mut vault = fixture(cx, &[("a.md", "#rust\n"), ("b.md", "")], "b.md");
    vault.change_tags(&rename_tag("rust", "lang"));
    assert_eq!(vault.seen(), [Seen::Show]);
    assert_eq!(vault.read("a.md"), "#rust\n");
    vault.press(ChangePreviewEvent::Apply);
    assert_eq!(
        vault.seen(),
        [Seen::Close {
            focus_sidebar: false
        }]
    );
    assert_eq!(vault.read("a.md"), "#lang\n");
    let undoable = vault.toast();
    vault.undo();
    assert_eq!(vault.read("a.md"), "#rust\n");
    assert!(
        !vault.toasts().contains(&undoable),
        "el aviso con Deshacer ya no vale"
    );
    assert_eq!(vault.toast(), "se deshizo: Renombrar #rust a #lang");
}

#[gpui_kit::test]
fn cancelar_cierra_y_no_cambia_nada(cx: &mut TestAppContext) {
    let mut vault = fixture(cx, &[("a.md", "#rust\n"), ("b.md", "")], "b.md");
    vault.change_tags(&rename_tag("rust", "lang"));
    vault.seen();
    vault.press(ChangePreviewEvent::Cancel);
    assert_eq!(
        vault.seen(),
        [Seen::Close {
            focus_sidebar: false
        }]
    );
    assert_eq!(vault.read("a.md"), "#rust\n");
    vault.undo();
    assert!(vault.toast().starts_with("no hay nada que deshacer"));
}

/// Abrir una línea deja la vista previa, y volver la rehace con lo quitado y
/// lo elegido como estaban. No se vuelve dos veces.
#[gpui_kit::test]
fn se_deja_para_abrir_una_linea_y_se_vuelve_a_ella(cx: &mut TestAppContext) {
    let notes = [("a.md", "uno\n#rust\n"), ("b.md", "#rust\n"), ("c.md", "")];
    let mut vault = fixture(cx, &notes, "c.md");
    vault.change_tags(&rename_tag("rust", "lang"));
    vault.seen();
    vault.view().update(vault.cx, |view, _| {
        view.preview_mut().toggle_selected();
    });
    vault.press(ChangePreviewEvent::OpenLine);
    assert_eq!(
        vault.seen(),
        [
            Seen::Close {
                focus_sidebar: false
            },
            Seen::OpenLine {
                path: "a.md".to_owned(),
                at: LinePosition { line: 1, column: 0 },
            },
        ]
    );
    assert_eq!(vault.toast(), "la vista previa sigue guardada");

    vault
        .changes
        .update(vault.cx, VaultChanges::resume_tag_change);
    assert_eq!(vault.seen(), [Seen::Show]);
    assert!(
        !vault
            .toasts()
            .contains(&"la vista previa sigue guardada".to_owned()),
        "el aviso de volver ya no vale"
    );
    let dropped = vault
        .view()
        .read_with(vault.cx, |view, _| view.preview().row(0).unwrap().dropped);
    assert!(dropped, "lo quitado sigue quitado");

    vault.press(ChangePreviewEvent::Cancel);
    vault
        .changes
        .update(vault.cx, VaultChanges::resume_tag_change);
    assert!(
        vault
            .toast()
            .starts_with("no hay vista previa a la que volver")
    );
}

/// Pedir otro cambio de tags olvida la vista previa que se había dejado.
#[gpui_kit::test]
fn otro_cambio_olvida_la_vista_previa_dejada(cx: &mut TestAppContext) {
    let mut vault = fixture(cx, &[("a.md", "#rust #go\n"), ("c.md", "")], "c.md");
    vault.change_tags(&rename_tag("rust", "lang"));
    vault.press(ChangePreviewEvent::OpenLine);
    vault.change_tags(&rename_tag("go", "golang"));
    assert!(
        !vault
            .toasts()
            .contains(&"la vista previa sigue guardada".to_owned()),
    );
    vault.press(ChangePreviewEvent::Cancel);
    vault
        .changes
        .update(vault.cx, VaultChanges::resume_tag_change);
    assert!(
        vault
            .toast()
            .starts_with("no hay vista previa a la que volver")
    );
}

/// Los buffers con cambios sin guardar reciben el cambio y el disco no; un
/// solo `:vaultundo` devuelve los dos.
#[gpui_kit::test]
fn los_tags_van_al_buffer_con_cambios(cx: &mut TestAppContext) {
    let mut vault = fixture(cx, &[("a.md", "#rust\n"), ("b.md", "#rust\n")], "a.md");
    vault.dirty("a.md");
    vault.change_tags(&rename_tag("rust", "lang"));
    vault.press(ChangePreviewEvent::Apply);
    assert_eq!(vault.read("a.md"), "#rust\n", "el disco no se toca");
    assert_eq!(vault.buffer("a.md"), "x\n#lang\n");
    assert_eq!(vault.read("b.md"), "#lang\n");
    vault.undo();
    assert!(vault.toast().starts_with("se deshizo"), "{}", vault.toast());
    assert_eq!(vault.buffer("a.md"), "x\n#rust\n");
    assert_eq!(vault.read("b.md"), "#rust\n");
}

/// La lista de notas sigue a un tag renombrado, y vuelve con el deshacer.
#[gpui_kit::test]
fn la_lista_sigue_al_tag_y_vuelve_con_el_deshacer(cx: &mut TestAppContext) {
    let mut vault = fixture(cx, &[("a.md", "#rust\n"), ("b.md", "")], "b.md");
    let source = |vault: &mut Fixture<'_>| {
        vault
            .list
            .read_with(vault.cx, |list, _| list.state().source)
    };
    vault.list.update(vault.cx, |list, cx| {
        list.set_source(ListSource::Tag("rust".to_owned()), cx);
    });
    vault.change_tags(&rename_tag("rust", "lang"));
    vault.press(ChangePreviewEvent::Apply);
    assert_eq!(source(&mut vault), ListSource::Tag("lang".to_owned()));
    vault.undo();
    assert_eq!(source(&mut vault), ListSource::Tag("rust".to_owned()));
}

/// Reescribir la config del vault se le avisa a la ventana, al aplicar y al
/// deshacer, y el renombre de una nota no.
#[gpui_kit::test]
fn reescribir_las_vistas_guardadas_se_avisa(cx: &mut TestAppContext) {
    let config = "views:\n  estudio: \"#rust\"\n";
    let notes = [
        ("a.md", "#rust\n"),
        ("b.md", ""),
        (".metalmind/config.yaml", config),
    ];
    let mut vault = fixture(cx, &notes, "b.md");
    vault.change_tags(&rename_tag("rust", "lang"));
    vault.seen();
    vault.press(ChangePreviewEvent::Apply);
    assert!(vault.seen().contains(&Seen::SavedViews));
    assert_eq!(
        vault.read(".metalmind/config.yaml"),
        config.replace("#rust", "#lang")
    );
    vault.undo();
    assert_eq!(vault.seen(), [Seen::SavedViews]);
    vault.plan_rename("b.md", "c.md");
    vault.finish_rename(false);
    assert_eq!(vault.seen(), [Seen::Renamed("c.md".to_owned())]);
}

/// Etiquetar desde la lista se aplica sin vista previa y se deshace como
/// cualquier cambio; lo que ya está se dice sin tocar nada.
#[gpui_kit::test]
fn etiquetar_una_nota_se_aplica_y_se_deshace(cx: &mut TestAppContext) {
    let mut vault = fixture(cx, &[("a.md", "uno\n"), ("b.md", "")], "b.md");
    let tags = ["rust".to_owned()];
    vault.changes.update(vault.cx, |changes, cx| {
        changes.tag_note(&path("a.md"), &tags, cx);
    });
    assert_eq!(vault.seen(), []);
    assert_eq!(vault.read("a.md"), "---\ntags: [rust]\n---\nuno\n");
    let tagged = vault.read("a.md");
    vault.changes.update(vault.cx, |changes, cx| {
        changes.tag_note(&path("a.md"), &tags, cx);
    });
    assert_eq!(vault.toast(), "a.md ya tiene rust");
    assert_eq!(vault.read("a.md"), tagged);
    vault.undo();
    assert_eq!(vault.read("a.md"), "uno\n");
}

/// Sin links que cambien se renombra enseguida, sin vista previa, y se
/// deshace igual.
#[gpui_kit::test]
fn un_renombre_sin_links_se_hace_enseguida(cx: &mut TestAppContext) {
    let mut vault = fixture(cx, &[("a.md", "uno\n"), ("b.md", "")], "b.md");
    vault.plan_rename("a.md", "c.md");
    vault.finish_rename(false);
    assert_eq!(vault.seen(), [Seen::Renamed("c.md".to_owned())]);
    assert!(vault.exists("c.md") && !vault.exists("a.md"));
    vault.undo();
    assert!(vault.exists("a.md") && !vault.exists("c.md"));
}

/// Con links que cambian: la vista previa, y al aplicar todo junto, con el
/// foco de vuelta en el panel lateral, de donde salió el renombre.
#[gpui_kit::test]
fn un_renombre_con_links_pasa_por_la_vista_previa(cx: &mut TestAppContext) {
    let notes = [("a.md", "[[b]]\n"), ("b.md", "# B\n"), ("c.md", "")];
    let mut vault = fixture(cx, &notes, "c.md");
    vault.plan_rename("b.md", "d.md");
    assert_eq!(vault.read("a.md"), "[[b]]\n", "antes de pedir seguir");
    vault.finish_rename(false);
    assert_eq!(vault.seen(), [Seen::Show]);
    assert_eq!(vault.read("a.md"), "[[b]]\n");
    vault.press(ChangePreviewEvent::Apply);
    assert_eq!(
        vault.seen(),
        [
            Seen::Close {
                focus_sidebar: true
            },
            Seen::Renamed("d.md".to_owned()),
        ]
    );
    assert_eq!(vault.read("a.md"), "[[d]]\n");
    assert!(vault.exists("d.md") && !vault.exists("b.md"));
    vault.undo();
    assert_eq!(vault.read("a.md"), "[[b]]\n");
    assert!(vault.exists("b.md"));
}

/// A lo que la ventana abrió mientras se armaba el plan no se le pasa por
/// encima: no se renombra, y se dice.
#[gpui_kit::test]
fn con_otra_cosa_abierta_no_se_renombra(cx: &mut TestAppContext) {
    let mut vault = fixture(cx, &[("a.md", "uno\n"), ("b.md", "")], "b.md");
    vault.plan_rename("a.md", "c.md");
    vault.finish_rename(true);
    assert_eq!(vault.seen(), []);
    assert!(vault.exists("a.md"));
    assert_eq!(
        vault.toast(),
        "no se renombró a.md: se abrió otra ventana antes de su vista previa"
    );
}

/// Un archivo del plan con cambios sin guardar bloquea el renombre, a
/// diferencia de los tags (03 §4), hasta que se guarda.
#[gpui_kit::test]
fn un_buffer_con_cambios_bloquea_el_renombre_hasta_guardarlo(cx: &mut TestAppContext) {
    let notes = [("a.md", "[[b]]\n"), ("b.md", "# B\n")];
    let mut vault = fixture(cx, &notes, "a.md");
    vault.dirty("a.md");
    vault.plan_rename("b.md", "d.md");
    vault.finish_rename(false);
    let blocked = |vault: &mut Fixture<'_>| {
        vault.view().read_with(vault.cx, |view, _| {
            matches!(view.preview().blocked(), Some(Blocked::Unsaved(_)))
        })
    };
    assert!(blocked(&mut vault));
    vault.workspace.update(vault.cx, |workspace, cx| {
        cx.notify();
        let save = Invocation {
            action: Command::Save.into(),
            argument: None,
        };
        let _ = workspace.run(&save, &mut SystemClipboard(cx));
    });
    vault.cx.run_until_parked();
    assert!(!blocked(&mut vault), "guardar lo destraba solo");
}

/// Mientras la vista previa es de un índice viejo no se aplica; al rehacerse,
/// sí, con lo quitado como estaba.
#[gpui_kit::test]
fn la_vista_previa_de_un_indice_viejo_se_rehace_antes_de_aplicar(cx: &mut TestAppContext) {
    let notes = [("a.md", "[[b]]\n"), ("b.md", "# B\n"), ("c.md", "[[b]]\n")];
    let mut vault = fixture(cx, &notes, "b.md");
    vault.plan_rename("b.md", "d.md");
    vault.finish_rename(false);
    vault.seen();
    vault.view().update(vault.cx, |view, _| {
        view.preview_mut().select(2);
        view.preview_mut().toggle_selected();
    });
    let latest = another_index(&notes);
    vault.changes.update(vault.cx, |changes, cx| {
        changes.refresh_rename_preview(&latest, cx);
    });
    vault.press(ChangePreviewEvent::Apply);
    assert_eq!(vault.seen(), [], "sigue abierta");
    assert!(
        vault
            .toast()
            .starts_with("la vista previa se está rehaciendo")
    );
    assert!(vault.exists("b.md"));

    vault.cx.run_until_parked();
    vault.press(ChangePreviewEvent::Apply);
    assert!(vault.exists("d.md"));
    assert_eq!(vault.read("a.md"), "[[d]]\n");
    assert_eq!(vault.read("c.md"), "[[b]]\n", "lo quitado sigue quitado");
}

/// Si la ventana cambió la vista previa por otra cosa, ya no se la rehace.
#[gpui_kit::test]
fn sin_la_vista_previa_en_la_ventana_no_se_rehace(cx: &mut TestAppContext) {
    let notes = [("a.md", "[[b]]\n"), ("b.md", "# B\n")];
    let mut vault = fixture(cx, &notes, "b.md");
    vault.plan_rename("b.md", "d.md");
    vault.finish_rename(false);
    assert!(
        vault
            .changes
            .update(vault.cx, |changes, _| changes.open_rename().is_some())
    );
    *vault.shown.borrow_mut() = None;
    vault.cx.run_until_parked();
    assert!(
        vault
            .changes
            .update(vault.cx, |changes, _| changes.open_rename().is_none())
    );
    let latest = another_index(&notes);
    vault.changes.update(vault.cx, |changes, cx| {
        changes.refresh_rename_preview(&latest, cx);
    });
    vault.cx.run_until_parked();
}
