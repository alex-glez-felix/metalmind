use std::cell::RefCell;
use std::fs;
use std::rc::Rc;

use gpui_kit::{Entity, TestAppContext, VisualTestContext};
use metalmind_config::Config;
use metalmind_keymap::Keymap;
use metalmind_vault::Vault;
use metalmind_vim::Markdown;
use pretty_assertions::assert_eq;

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
    let dir = tempfile::tempdir().unwrap();
    for (name, text) in files {
        fs::write(dir.path().join(name), text).unwrap();
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
    let state = ListState {
        visible,
        ..ListState::default()
    };
    let (list, cx) = cx.add_window_view(|_, cx| NoteListView::new(&session, &state, workspace, cx));
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
fn al_abrirla_elige_la_nota_activa() {
    let mut cx = TestAppContext::single();
    let (fixture, cx) = fixture(&mut cx);
    assert!(!fixture.list.read_with(cx, |view, _| view.is_visible()));
    assert!(fixture.list.update(cx, NoteListView::toggle));
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
        fixture.list.update(cx, |view, _| {
            view.chosen = chosen;
            view.by_width = None;
            for &width in widths {
                view.settle_visibility(gpui_kit::px(width), min);
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
