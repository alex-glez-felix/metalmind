//! La ventana cumple lo que le pide `VaultChanges`: abre y cierra la vista
//! previa, mueve el foco y sigue a lo renombrado.

use std::fs;
use std::sync::mpsc;

use gpui_kit::TestAppContext;
use metalmind_vault::{Vault, VaultPath, VaultUpdate};
use pretty_assertions::assert_eq;

use super::board_tests::{Window, open_prepared};
use super::palette::Palette;
use crate::finder::Prepared;
use crate::sidebar::SidebarEvent;

/// Una ventana sobre `notes`, con `file` abierto; los lotes del watcher se
/// quedan en el canal, sin leer.
fn open<'a>(
    cx: &'a mut TestAppContext,
    notes: &[(&str, &str)],
    file: &str,
) -> (Window<'a>, mpsc::Receiver<VaultUpdate>) {
    let dir = tempfile::tempdir().unwrap();
    for (name, text) in notes {
        fs::write(dir.path().join(name), text).unwrap();
    }
    let (vault, watcher) = Vault::open(dir.path()).unwrap();
    let prepared = Prepared::without_updates(vault);
    (open_prepared(cx, dir, prepared, file), watcher)
}

impl Window<'_> {
    fn command(&mut self, command: &str) {
        self.keys(":");
        self.cx.simulate_input(command);
        self.keys("enter");
    }

    fn overlay_open(&mut self) -> bool {
        self.view
            .read_with(self.cx, |view, _| view.overlay.is_some())
    }

    fn sidebar_focused(&mut self) -> bool {
        self.view.update_in(self.cx, |view, window, cx| {
            view.sidebar
                .read(cx)
                .focus_handle()
                .contains_focused(window, cx)
        })
    }

    fn rename(&mut self, from: &str, to: &str) {
        self.view.update_in(self.cx, |view, window, cx| {
            let event = SidebarEvent::Rename {
                from: VaultPath::new(from).unwrap(),
                to: VaultPath::new(to).unwrap(),
            };
            view.on_sidebar_event(event, window, cx);
        });
        self.cx.run_until_parked();
    }

    fn read(&self, name: &str) -> String {
        fs::read_to_string(self.root.join(name)).unwrap()
    }
}

#[gpui_kit::test]
fn un_cambio_de_tags_abre_su_vista_previa_y_enter_la_aplica(cx: &mut TestAppContext) {
    let notes = [("uno.md", "#rust\n"), ("dos.md", "")];
    let (mut window, _watcher) = open(cx, &notes, "dos.md");
    window.command("tagrename #rust #lang");
    assert!(window.overlay_open());
    assert_eq!(window.read("uno.md"), "#rust\n");
    window.keys("enter");
    assert!(!window.overlay_open());
    assert_eq!(window.read("uno.md"), "#lang\n");
    window.command("vaultundo");
    assert_eq!(window.read("uno.md"), "#rust\n");
}

/// `o` deja la vista previa y abre la línea; `space r` vuelve a ella.
#[gpui_kit::test]
fn abrir_una_linea_y_volver_a_la_vista_previa(cx: &mut TestAppContext) {
    let notes = [("uno.md", "#rust\n"), ("dos.md", "")];
    let (mut window, _watcher) = open(cx, &notes, "dos.md");
    window.command("tagrename #rust #lang");
    window.keys("o");
    assert!(!window.overlay_open());
    assert_eq!(window.file(), "uno.md");
    window.keys("space r");
    assert!(window.overlay_open());
    window.keys("escape");
    assert!(!window.overlay_open());
}

/// Un renombre con links abre su vista previa; al aplicarla, el foco vuelve al
/// panel lateral y la nota abierta sigue al archivo.
#[gpui_kit::test]
fn un_renombre_con_links_vuelve_al_panel_y_la_nota_lo_sigue(cx: &mut TestAppContext) {
    let notes = [("uno.md", "[[dos]]\n"), ("dos.md", "# Dos\n")];
    let (mut window, _watcher) = open(cx, &notes, "dos.md");
    window.rename("dos.md", "tres.md");
    assert!(window.overlay_open());
    window.keys("enter");
    assert!(!window.overlay_open());
    assert!(window.sidebar_focused());
    assert_eq!(window.read("uno.md"), "[[tres]]\n");
    assert_eq!(window.file(), "tres.md");
}

/// Lo que se abrió mientras se armaba el plan se queda abierto, y no se
/// renombra.
#[gpui_kit::test]
fn un_renombre_no_le_pasa_por_encima_a_lo_abierto(cx: &mut TestAppContext) {
    let notes = [("uno.md", "[[dos]]\n"), ("dos.md", "# Dos\n")];
    let (mut window, _watcher) = open(cx, &notes, "uno.md");
    window.view.update_in(window.cx, |view, window, cx| {
        let event = SidebarEvent::Rename {
            from: VaultPath::new("dos.md").unwrap(),
            to: VaultPath::new("tres.md").unwrap(),
        };
        view.on_sidebar_event(event, window, cx);
        view.open_palette(window, cx);
    });
    window.cx.run_until_parked();
    assert!(window.overlay_open());
    assert!(window.root.join("dos.md").exists());
    let open = window
        .view
        .read_with(window.cx, |view, _| view.overlay.as_ref().unwrap().view());
    assert!(open.downcast::<Palette>().is_ok(), "sigue la paleta");
}
