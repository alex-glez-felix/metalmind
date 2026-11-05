//! El panel sobre un vault con una nota, para los tests de sus vistas.

use std::fs;

use gpui_kit::{AppContext as _, Entity, TestAppContext, VisualTestContext};
use metalmind_config::Config;
use metalmind_keymap::Keymap;
use metalmind_vault::Vault;
use metalmind_vim::Markdown;

use super::Sidebar;
use crate::actions::bind_keys;
use crate::document::Document;
use crate::finder::{Prepared, VaultSession};
use crate::session::PanelState;
use crate::theme::Theme;
use crate::workspace::{LineNumbers, Options, PaneOptions, Workspace};

pub(super) fn sidebar(
    cx: &mut TestAppContext,
) -> (tempfile::TempDir, Entity<Sidebar>, &mut VisualTestContext) {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.md"), "Ah.").unwrap();
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
    let document = Document::open(&root.join("a.md")).unwrap();
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
    let state = PanelState::default();
    let (sidebar, cx) = cx.add_window_view(|_, cx| Sidebar::new(&session, &state, workspace, cx));
    cx.run_until_parked();
    cx.update(|window, cx| {
        let handle = sidebar.read(cx).focus_handle().clone();
        window.focus(&handle, cx);
    });
    (dir, sidebar, cx)
}
