use std::fs;
use std::time::Duration;

use gpui_kit::{AppContext as _, Entity, TestAppContext, VisualTestContext};
use metalmind_config::{Config, WhichKey as Settings};
use metalmind_keymap::Keymap;
use metalmind_vault::Vault;
use metalmind_vim::Markdown;

use super::super::Editor;
use crate::actions::bind_keys;
use crate::document::Document;
use crate::finder::{Prepared, VaultSession};
use crate::math::Formulas;
use crate::theme::Theme;
use crate::workspace::{LineNumbers, Options, PaneOptions, Workspace};

const DELAY: Duration = Duration::from_millis(500);
const JUST_BEFORE: Duration = Duration::from_millis(499);

/// Un editor con el foco sobre `a.md`, con `which_key` así.
fn editor(
    cx: &mut TestAppContext,
    which_key: Settings,
) -> (tempfile::TempDir, &mut VisualTestContext) {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.md"), "uno\ndos\n").unwrap();
    let (vault, updates) = Vault::open(dir.path()).unwrap();
    let path = vault.root().join("a.md");
    cx.update(|cx| {
        cx.set_global(Theme::with_font(
            &Config::defaults().unwrap(),
            "Mono".into(),
        ));
        bind_keys(Keymap::default(), cx);
    });
    let prepared = Prepared::new(vault, updates, None).unwrap();
    let session = cx.update(|cx| cx.new(|cx| VaultSession::new(prepared, cx)));
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
            let document = Document::open(&path).unwrap();
            let mut workspace = Workspace::new(document, pane, options, &Keymap::default());
            workspace.set_which_key(which_key);
            workspace
        })
    });
    let formulas = cx.update(|cx| cx.new(|_| Formulas::default()));
    let pane = workspace.read_with(cx, |workspace, _| workspace.focused());
    let (editor, cx) =
        cx.add_window_view(|_, cx| Editor::new(workspace, session, formulas, pane, cx));
    focus(&editor, cx);
    (dir, cx)
}

fn focus(editor: &Entity<Editor>, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        let handle = editor.read(cx).focus_handle().clone();
        window.focus(&handle, cx);
    });
    cx.run_until_parked();
}

fn shown(cx: &mut VisualTestContext) -> bool {
    cx.run_until_parked();
    cx.debug_bounds("which-key").is_some()
}

#[test]
fn sale_tras_el_retraso_y_se_va_con_la_siguiente_tecla() {
    let mut cx = TestAppContext::single();
    let (_dir, cx) = editor(&mut cx, Settings::default());
    cx.simulate_keystrokes("space");
    assert!(!shown(cx));
    cx.executor().advance_clock(JUST_BEFORE);
    assert!(!shown(cx));
    cx.executor().advance_clock(Duration::from_millis(1));
    assert!(shown(cx));
    cx.simulate_keystrokes("escape");
    assert!(!shown(cx));
}

/// Tecleando rápido no se alcanza a ver.
#[test]
fn no_sale_si_el_atajo_se_completa_antes() {
    let mut cx = TestAppContext::single();
    let (_dir, cx) = editor(&mut cx, Settings::default());
    cx.simulate_keystrokes("g");
    cx.executor().advance_clock(DELAY / 2);
    cx.simulate_keystrokes("g");
    cx.executor().advance_clock(DELAY);
    assert!(!shown(cx));
}

#[test]
fn apagado_no_sale() {
    let mut cx = TestAppContext::single();
    let off = Settings {
        enabled: false,
        ..Settings::default()
    };
    let (_dir, cx) = editor(&mut cx, off);
    cx.simulate_keystrokes("space");
    cx.executor().advance_clock(DELAY * 4);
    assert!(!shown(cx));
}
