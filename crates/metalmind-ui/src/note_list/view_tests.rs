use std::cell::RefCell;
use std::fs::{self, File};
use std::rc::Rc;
use std::time::{Duration, SystemTime};

use gpui_kit::{Entity, TestAppContext, VisualTestContext};
use metalmind_config::{Config, SavedView};
use metalmind_keymap::Keymap;
use metalmind_vault::Vault;
use metalmind_vim::Markdown;
use pretty_assertions::assert_eq;

use gpui_kit::EntityInputHandler as _;

use super::*;
use crate::actions::bind_keys;
use crate::document::Document;
use crate::finder::Prepared;
use crate::theme::Theme;
use crate::workspace::{LineNumbers, Options, PaneOptions};

struct Fixture {
    _dir: tempfile::TempDir,
    list: Entity<NoteListView>,
    events: Rc<RefCell<Vec<NoteListEvent>>>,
}

/// De la más reciente a la más vieja: es el orden de la lista.
const FILES: &[(&str, &str)] = &[("a.md", "#rust"), ("b.md", "# Be\nTexto."), ("c.md", "Ce.")];

/// Una lista plegada del buzón con `b.md` abierta en el panel de edición.
fn fixture(cx: &mut TestAppContext) -> (Fixture, &mut VisualTestContext) {
    fixture_with(cx, FILES, Some(false))
}

fn fixture_with<'c>(
    cx: &'c mut TestAppContext,
    files: &[(&str, &str)],
    visible: Option<bool>,
) -> (Fixture, &'c mut VisualTestContext) {
    let state = ListState {
        visible,
        ..ListState::default()
    };
    fixture_state(cx, files, &state)
}

fn fixture_state<'c>(
    cx: &'c mut TestAppContext,
    files: &[(&str, &str)],
    state: &ListState,
) -> (Fixture, &'c mut VisualTestContext) {
    let dir = tempfile::tempdir().unwrap();
    let now = SystemTime::now();
    // Sin fechas explícitas, escrituras seguidas pueden caer en el mismo tick
    // del reloj del sistema de archivos o no, y el orden cambiaría.
    for ((name, text), minutes_ago) in files.iter().zip(0..) {
        let path = dir.path().join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, text).unwrap();
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(now - Duration::from_secs(minutes_ago * 60))
            .unwrap();
    }
    let (vault, updates) = Vault::open(dir.path()).unwrap();
    let root = vault.root().to_owned();
    cx.update(|cx| {
        cx.set_global(Theme::with_font(
            &Config::defaults().unwrap(),
            "Mono".into(),
        ));
        bind_keys(Keymap::default(), cx);
    });
    let prepared = Prepared::new(vault, updates, None).unwrap();
    let session = cx.update(|cx| cx.new(|cx| VaultSession::new(prepared, cx)));
    let document = Document::open(&root.join("b.md")).unwrap_or_else(|_| Document::unnamed());
    let options = Options {
        scrolloff: 0,
        tabstop: 8,
        expandtab: false,
        gutter_marks: false,
        markdown: Markdown::default(),
    };
    let workspace = cx.update(|cx| {
        cx.new(|_| {
            let pane = PaneOptions {
                numbers: LineNumbers::HYBRID,
                wrap: true,
            };
            Workspace::new(document, pane, options, &Keymap::default())
        })
    });
    let (list, cx) = cx.add_window_view(|_, cx| NoteListView::new(&session, state, workspace, cx));
    let events = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::clone(&events);
    cx.update(|_, cx| {
        cx.subscribe(&list, move |_, event: &NoteListEvent, _| {
            seen.borrow_mut().push(event.clone());
        })
        .detach();
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        let handle = list.read(cx).focus_handle().clone();
        window.focus(&handle, cx);
    });
    let fixture = Fixture {
        _dir: dir,
        list,
        events,
    };
    (fixture, cx)
}

fn selected(list: &Entity<NoteListView>, cx: &mut VisualTestContext) -> Option<String> {
    list.read_with(cx, |view, _| {
        view.list.selected_note().map(ToString::to_string)
    })
}

#[test]
fn plegada_no_lee_ninguna_tarjeta() {
    let mut cx = TestAppContext::single();
    let (fixture, cx) = fixture(&mut cx);
    let (started, current) = fixture
        .list
        .read_with(cx, |view, _| (view.started, view.list.is_current()));
    assert!(!started && !current);
    fixture.list.update(cx, NoteListView::toggle);
    cx.run_until_parked();
    cx.run_until_parked();
    assert!(fixture.list.read_with(cx, |view, _| view.list.is_current()));
}

#[test]
fn al_abrirla_elige_la_nota_activa() {
    let mut cx = TestAppContext::single();
    let (fixture, cx) = fixture(&mut cx);
    assert!(!fixture.list.read_with(cx, |view, _| view.is_visible()));
    assert!(fixture.list.update(cx, NoteListView::toggle));
    cx.run_until_parked();
    assert_eq!(selected(&fixture.list, cx).as_deref(), Some("b.md"));
    assert_eq!(
        fixture.list.read_with(cx, |view, _| view.state().visible),
        Some(true)
    );
}

#[test]
fn t_pregunta_los_tags_de_la_nota_elegida() {
    let mut cx = TestAppContext::single();
    let (fixture, cx) = fixture(&mut cx);
    fixture.list.update(cx, NoteListView::toggle);
    cx.run_until_parked();
    cx.simulate_keystrokes("t");
    cx.simulate_input("idea gpui");
    cx.simulate_keystrokes("enter");
    let events = fixture.events.borrow().clone();
    assert_eq!(
        events,
        [NoteListEvent::TagNote {
            note: VaultPath::new("b.md").unwrap(),
            tags: vec!["idea".to_owned(), "gpui".to_owned()],
        }]
    );
}

#[test]
fn el_filtro_se_escribe_en_su_lugar_y_esc_lo_quita() {
    let mut cx = TestAppContext::single();
    let (fixture, cx) = fixture(&mut cx);
    fixture.list.update(cx, NoteListView::toggle);
    cx.run_until_parked();
    cx.simulate_keystrokes("space f");
    cx.simulate_input("c.m");
    cx.run_until_parked();
    cx.simulate_keystrokes("enter");
    let rows = |cx: &mut VisualTestContext| {
        fixture.list.read_with(cx, |view, _| {
            (view.list.len(), view.list.filter().text().to_owned())
        })
    };
    assert_eq!(rows(cx), (1, "c.m".to_owned()));
    assert_eq!(selected(&fixture.list, cx).as_deref(), Some("c.md"));
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(rows(cx), (2, String::new()));
}

#[test]
fn enter_abre_la_nota_y_pasa_el_foco() {
    let mut cx = TestAppContext::single();
    let (fixture, cx) = fixture(&mut cx);
    fixture.list.update(cx, NoteListView::toggle);
    cx.run_until_parked();
    cx.simulate_keystrokes("j enter");
    assert_eq!(
        *fixture.events.borrow(),
        [NoteListEvent::Opened { focus_editor: true }]
    );
    let active = fixture.list.read_with(cx, NoteListView::active);
    assert_eq!(
        active,
        selected(&fixture.list, cx).and_then(|path| VaultPath::new(&path).ok())
    );
}

#[test]
fn sin_estado_guardado_arranca_abierta_solo_en_una_ventana_ancha() {
    let mut cx = TestAppContext::single();
    let (fixture, cx) = fixture(&mut cx);
    let min = gpui_kit::px(1600.);
    let settle = |chosen: Option<bool>, widths: &[f32], cx: &mut VisualTestContext| {
        fixture.list.update(cx, |view, cx| {
            view.chosen = chosen;
            view.by_width = None;
            for &width in widths {
                view.settle_visibility(gpui_kit::px(width), min, cx);
            }
            (view.is_visible(), view.state().visible)
        })
    };
    assert_eq!(settle(None, &[1920.], cx), (true, None), "no se guarda");
    assert_eq!(settle(None, &[1280.], cx), (false, None));
    assert_eq!(
        settle(None, &[1280., 1920.], cx),
        (false, None),
        "no brinca"
    );
    assert_eq!(
        settle(Some(true), &[1280.], cx),
        (true, Some(true)),
        "lo elegido gana"
    );
    assert_eq!(settle(Some(false), &[1920.], cx), (false, Some(false)));
}

#[test]
fn abierta_al_arrancar_elige_la_nota_activa_cuando_llegan_las_tarjetas() {
    let mut cx = TestAppContext::single();
    let (fixture, cx) = fixture_with(&mut cx, FILES, Some(true));
    assert_eq!(selected(&fixture.list, cx).as_deref(), Some("b.md"));
}

#[test]
fn un_vault_sin_notas_dice_que_no_hay_ninguna() {
    let mut cx = TestAppContext::single();
    let (fixture, cx) = fixture_with(&mut cx, &[], Some(true));
    let (current, total) = fixture
        .list
        .read_with(cx, |view, _| (view.list.is_current(), view.list.total()));
    assert!(current, "sin notas, la lista ya está");
    assert_eq!(total, 0);
}

#[test]
fn una_fuente_elegida_gana_a_la_nota_activa_al_cargar() {
    let mut cx = TestAppContext::single();
    let (fixture, cx) = fixture(&mut cx);
    fixture
        .list
        .update(cx, |view, cx| view.set_source(ListSource::All, cx));
    cx.run_until_parked();
    assert_eq!(selected(&fixture.list, cx).as_deref(), Some("a.md"));
    assert_eq!(
        fixture.list.read_with(cx, |view, _| view.source_choices()),
        1
    );
}

#[test]
fn el_filtro_recibe_el_texto_del_ime() {
    let mut cx = TestAppContext::single();
    let (fixture, cx) = fixture(&mut cx);
    fixture.list.update(cx, NoteListView::toggle);
    cx.run_until_parked();
    cx.simulate_keystrokes("space f");
    fixture.list.update_in(cx, |view, window, cx| {
        view.replace_and_mark_text_in_range(None, "´", None, window, cx);
        view.replace_text_in_range(None, "é", window, cx);
    });
    let text = fixture
        .list
        .read_with(cx, |view, _| view.list.filter().text().to_owned());
    assert_eq!(text, "é");
}

#[test]
fn cerrar_la_pregunta_suelta_el_preedit() {
    let mut cx = TestAppContext::single();
    let (fixture, cx) = fixture(&mut cx);
    fixture.list.update(cx, NoteListView::toggle);
    cx.run_until_parked();
    cx.simulate_keystrokes("t");
    fixture.list.update_in(cx, |view, window, cx| {
        view.replace_and_mark_text_in_range(None, "´", None, window, cx);
    });
    assert_eq!(
        fixture
            .list
            .read_with(cx, |view, _| view.marked().to_owned()),
        "´"
    );
    cx.simulate_keystrokes("escape");
    assert_eq!(
        fixture
            .list
            .read_with(cx, |view, _| view.marked().to_owned()),
        ""
    );
}

#[test]
fn la_carpeta_de_la_nota_activa_llega_con_ella_elegida() {
    let mut cx = TestAppContext::single();
    let files = &[
        ("a.md", "#rust"),
        ("b.md", "# Be"),
        ("ui/c.md", "Ce."),
        ("ui/hondo/d.md", "De."),
    ];
    let (fixture, cx) = fixture_with(&mut cx, files, Some(false));
    let show = |named: Option<&str>, cx: &mut VisualTestContext| {
        let shown = fixture.list.update(cx, |view, cx| {
            view.show_folder(named, cx)
                .map_err(|error| error.to_string())
        });
        cx.run_until_parked();
        shown
    };
    // `b.md` está en la raíz: su carpeta son todas las notas, y se elige
    // aunque no sea la primera fila.
    show(None, cx).unwrap();
    let source = fixture
        .list
        .read_with(cx, |view, _| view.list.source().clone());
    assert_eq!(source, ListSource::All);
    assert!(fixture.list.read_with(cx, |view, _| view.is_visible()));
    assert_eq!(selected(&fixture.list, cx).as_deref(), Some("b.md"));
    // Una miga: la nota activa no es de ahí, así que la primera fila.
    show(Some("ui"), cx).unwrap();
    let source = fixture
        .list
        .read_with(cx, |view, _| view.list.source().clone());
    assert_eq!(source, ListSource::Folder("ui".into()));
    assert_eq!(selected(&fixture.list, cx).as_deref(), Some("ui/c.md"));
    assert_eq!(
        show(Some("nada"), cx),
        Err("nada no es una carpeta del vault".to_owned())
    );
    let source = fixture
        .list
        .read_with(cx, |view, _| view.list.source().clone());
    assert_eq!(
        source,
        ListSource::Folder("ui".into()),
        "un error no cambia la fuente"
    );
}

fn shown(list: &Entity<NoteListView>, cx: &mut VisualTestContext) -> (ListSource, Vec<String>) {
    list.read_with(cx, |view, _| {
        let notes = (0..view.list.len())
            .filter_map(|row| view.list.note(row).map(ToString::to_string))
            .collect();
        (view.list.source().clone(), notes)
    })
}

const WITH_VIEWS: &[(&str, &str)] = &[
    ("a.md", "#rust"),
    ("b.md", "#rust #archivado"),
    ("c.md", "Ce."),
    (
        ".metalmind/config.yaml",
        "views:\n  gs: \"#rust -#archivado\"\n",
    ),
];

#[test]
fn una_vista_guardada_vuelve_con_la_consulta_del_archivo() {
    let mut cx = TestAppContext::single();
    let state = ListState {
        visible: Some(true),
        source: ListSource::View {
            name: "gs".into(),
            query: "#viejo".into(),
        },
        filter: String::new(),
    };
    let (fixture, cx) = fixture_state(&mut cx, WITH_VIEWS, &state);
    let gs = ListSource::View {
        name: "gs".into(),
        query: "#rust -#archivado".into(),
    };
    assert_eq!(shown(&fixture.list, cx), (gs, vec!["a.md".to_owned()]));
}

#[test]
fn una_vista_que_ya_no_esta_vuelve_al_buzon() {
    let mut cx = TestAppContext::single();
    let state = ListState {
        visible: Some(true),
        source: ListSource::View {
            name: "otra".into(),
            query: "#rust".into(),
        },
        filter: String::new(),
    };
    let (fixture, cx) = fixture_state(&mut cx, WITH_VIEWS, &state);
    assert_eq!(
        shown(&fixture.list, cx),
        (ListSource::Inbox, vec!["c.md".to_owned()])
    );
}

#[test]
fn la_vista_recien_guardada_se_muestra_sin_el_filtro() {
    let mut cx = TestAppContext::single();
    let (fixture, cx) = fixture_with(&mut cx, WITH_VIEWS, Some(true));
    cx.simulate_keystrokes("space f");
    cx.simulate_input("#rust");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    let view = SavedView {
        name: "Rust".into(),
        query: "#rust".into(),
    };
    fixture
        .list
        .update(cx, |list, cx| list.show_saved_view(&view, cx));
    cx.run_until_parked();
    let (source, notes) = shown(&fixture.list, cx);
    assert_eq!(source, ListSource::of_view(&view));
    assert_eq!(notes, ["a.md", "b.md"]);
    let filter = fixture
        .list
        .read_with(cx, |view, _| view.list.filter().text().to_owned());
    assert_eq!(filter, "");
}

#[test]
fn la_lista_sigue_a_su_vista_renombrada_o_borrada() {
    let mut cx = TestAppContext::single();
    let gs = SavedView {
        name: "gs".into(),
        query: "#rust -#archivado".into(),
    };
    let state = ListState {
        visible: Some(true),
        source: ListSource::of_view(&gs),
        filter: String::new(),
    };
    let (fixture, cx) = fixture_state(&mut cx, WITH_VIEWS, &state);
    let estudio = SavedView {
        name: "estudio".into(),
        ..gs
    };
    fixture.list.update(cx, |list, cx| {
        list.follow_views(std::slice::from_ref(&estudio), Some(("gs", "estudio")), cx);
    });
    cx.run_until_parked();
    assert_eq!(
        shown(&fixture.list, cx),
        (ListSource::of_view(&estudio), vec!["a.md".to_owned()])
    );
    fixture
        .list
        .update(cx, |list, cx| list.follow_views(&[], None, cx));
    cx.run_until_parked();
    assert_eq!(
        shown(&fixture.list, cx),
        (ListSource::Inbox, vec!["c.md".to_owned()])
    );
}

#[test]
fn con_la_config_rota_la_vista_guardada_no_se_pierde() {
    let mut cx = TestAppContext::single();
    let gs = ListSource::View {
        name: "gs".into(),
        query: "#rust -#archivado".into(),
    };
    let state = ListState {
        visible: Some(true),
        source: gs.clone(),
        filter: String::new(),
    };
    let files = [
        ("a.md", "#rust"),
        ("b.md", "#rust #archivado"),
        (".metalmind/config.yaml", "views: [\n"),
    ];
    let (fixture, cx) = fixture_state(&mut cx, &files, &state);
    assert_eq!(shown(&fixture.list, cx), (gs, vec!["a.md".to_owned()]));
}
