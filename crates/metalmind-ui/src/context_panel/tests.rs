use std::cell::RefCell;
use std::fs;
use std::rc::Rc;

use gpui_kit::{AppContext as _, Entity, TestAppContext, VisualTestContext};
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
    panel: Entity<ContextPanel>,
    workspace: Entity<Workspace>,
    events: Rc<RefCell<Vec<ContextEvent>>>,
}

/// `b.md`, abierta en el panel de edición, recibe links de `a.md` y `c.md`
/// y enlaza a `c.md` y a una nota que no existe.
const FILES: &[(&str, &str)] = &[
    ("a.md", "uno\nver [[b]] y [[c]]\n"),
    ("b.md", "# Be\nver [[c]] y [[falta]]\n"),
    ("c.md", "Ce [[b]]\n"),
];

/// El panel cerrado, junto a un workspace con `b.md`.
fn fixture(cx: &mut TestAppContext) -> (Fixture, &mut VisualTestContext) {
    let dir = tempfile::tempdir().unwrap();
    for (name, text) in FILES {
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
    let document = Document::open(&root.join("b.md")).unwrap();
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
    let shared = workspace.clone();
    let (panel, cx) =
        cx.add_window_view(|window, cx| ContextPanel::new(&session, shared, window, cx));
    let events = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::clone(&events);
    cx.update(|_, cx| {
        cx.subscribe(&panel, move |_, event: &ContextEvent, _| {
            seen.borrow_mut().push(event.clone());
        })
        .detach();
    });
    cx.run_until_parked();
    let fixture = Fixture {
        _dir: dir,
        panel,
        workspace,
        events,
    };
    (fixture, cx)
}

/// Lo abre como `space b` y le da el foco, como hace la ventana.
fn open(fixture: &Fixture, cx: &mut VisualTestContext) {
    let visible = fixture
        .panel
        .update_in(cx, |panel, window, cx| panel.toggle(window, cx));
    assert!(visible);
    cx.update(|window, cx| {
        let handle = fixture.panel.read(cx).focus_handle().clone();
        window.focus(&handle, cx);
    });
}

fn active(fixture: &Fixture, cx: &mut VisualTestContext) -> String {
    fixture.workspace.read_with(cx, |workspace, _| {
        let path = workspace.focused_document().unwrap().path().unwrap();
        path.file_name().unwrap().to_string_lossy().into_owned()
    })
}

fn counts(fixture: &Fixture, cx: &mut VisualTestContext) -> (usize, usize) {
    fixture
        .panel
        .read_with(cx, |panel, _| panel.context.counts())
}

#[test]
fn cerrado_hasta_que_se_pide_y_entonces_muestra_la_nota_activa() {
    let mut cx = TestAppContext::single();
    let (fixture, cx) = fixture(&mut cx);
    assert!(!fixture.panel.read_with(cx, |panel, _| panel.is_visible()));
    open(&fixture, cx);
    let note = fixture
        .panel
        .read_with(cx, |panel, _| panel.context.note().cloned());
    assert_eq!(note, Some(VaultPath::new("b.md").unwrap()));
    assert_eq!(counts(&fixture, cx), (2, 2));
}

/// `l` baja de la nota a su link y `enter` abre la nota en él y le pasa el
/// foco al editor.
#[test]
fn enter_abre_el_backlink_en_su_link() {
    let mut cx = TestAppContext::single();
    let (fixture, cx) = fixture(&mut cx);
    open(&fixture, cx);
    cx.simulate_keystrokes("l enter");
    assert_eq!(active(&fixture, cx), "a.md");
    let cursor = fixture.workspace.read_with(cx, |workspace, _| {
        workspace.view(workspace.focused()).unwrap().cursor()
    });
    assert_eq!(cursor, metalmind_buffer::ByteOffset::new(8));
    assert_eq!(
        fixture.events.borrow().clone(),
        [ContextEvent::Opened { focus_editor: true }]
    );
}

/// `tab` pasa a salientes, donde `enter` abre el destino.
#[test]
fn tab_cambia_a_salientes() {
    let mut cx = TestAppContext::single();
    let (fixture, cx) = fixture(&mut cx);
    open(&fixture, cx);
    cx.simulate_keystrokes("tab");
    let tab = fixture.panel.read_with(cx, |panel, _| panel.context.tab());
    assert_eq!(tab, ContextTab::Outgoing);
    cx.simulate_keystrokes("enter");
    assert_eq!(active(&fixture, cx), "c.md");
}

/// Un clic simple abre la nota y deja el foco en el panel, que se queda en
/// su nota para recorrerla; cerrarlo lo suelta.
#[test]
fn un_clic_abre_sin_soltar_la_nota_del_panel() {
    let mut cx = TestAppContext::single();
    let (fixture, cx) = fixture(&mut cx);
    open(&fixture, cx);
    fixture
        .panel
        .update_in(cx, |panel, window, cx| panel.click(2, 1, window, cx));
    cx.run_until_parked();
    assert_eq!(active(&fixture, cx), "c.md");
    let shown = fixture
        .panel
        .read_with(cx, |panel, _| panel.context.note().cloned());
    assert_eq!(shown, Some(VaultPath::new("b.md").unwrap()));
    assert_eq!(
        fixture.events.borrow().clone(),
        [ContextEvent::Opened {
            focus_editor: false
        }]
    );
    cx.simulate_keystrokes("escape");
    assert!(!fixture.panel.read_with(cx, |panel, _| panel.is_visible()));
    open(&fixture, cx);
    let shown = fixture
        .panel
        .read_with(cx, |panel, _| panel.context.note().cloned());
    assert_eq!(shown, Some(VaultPath::new("c.md").unwrap()));
}

/// `esc` lo cierra y le devuelve el foco al editor; `space b` con el foco
/// en el panel, también.
#[test]
fn esc_y_space_b_lo_cierran() {
    let mut cx = TestAppContext::single();
    let (fixture, cx) = fixture(&mut cx);
    open(&fixture, cx);
    cx.simulate_keystrokes("escape");
    assert!(!fixture.panel.read_with(cx, |panel, _| panel.is_visible()));
    assert_eq!(fixture.events.borrow().clone(), [ContextEvent::FocusEditor]);
    open(&fixture, cx);
    let visible = fixture
        .panel
        .update_in(cx, |panel, window, cx| panel.toggle(window, cx));
    assert!(!visible);
}
