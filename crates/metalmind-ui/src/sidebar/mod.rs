//! El panel lateral: el árbol de carpetas del vault, que se actualiza con
//! cada cambio que trae la [`VaultSession`] de la ventana, y las
//! operaciones de archivo. La lógica vive en
//! [`Panel`](crate::panel::Panel) y en el [`Workspace`]; aquí se pinta, se
//! traduce la entrada y se ejecuta lo que el usuario confirmó.

mod actions;
mod handlers;
mod icons;
mod input;
mod menu;
mod prompt;
mod rail;
mod render;

use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::{
    App, Context, Entity, EventEmitter, FocusHandle, ScrollStrategy, Subscription,
    UniformListScrollHandle,
};
use metalmind_vault::{Vault, VaultPath, VaultUpdate};

pub use self::actions::bind_keys;
use self::menu::ContextMenu;
pub use self::rail::rail;
use crate::file_tree::FileTree;
use crate::finder::{VaultChanged, VaultSession};
use crate::panel::{Panel, PanelView};
use crate::session::PanelState;
use crate::workspace::{Placement, Workspace, shown_path};

/// Lo que la ventana tiene que hacer por el panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarEvent {
    /// Se abrió un archivo en el panel de edición enfocado: pasa el foco ahí.
    Opened,
    /// El foco vuelve al panel de edición.
    FocusEditor,
    /// Se cerraron paneles de edición (al borrar lo que mostraban); el
    /// foco se queda en el panel lateral.
    PanesChanged,
}

pub struct Sidebar {
    panel: Panel,
    vault: Rc<Vault>,
    workspace: Entity<Workspace>,
    focus_handle: FocusHandle,
    scroll: UniformListScrollHandle,
    menu: Option<ContextMenu>,
    _changes: Subscription,
}

impl EventEmitter<SidebarEvent> for Sidebar {}

impl Sidebar {
    /// El panel del vault de `session` como lo dejó `state`.
    pub fn new(
        session: &Entity<VaultSession>,
        state: &PanelState,
        workspace: Entity<Workspace>,
        cx: &mut Context<Self>,
    ) -> Self {
        let vault = session.read(cx).vault();
        let tree = FileTree::with_expanded(vault.snapshot(), state.expanded());
        let changes = cx.subscribe(session, |sidebar, _, VaultChanged(update), cx| {
            sidebar.apply(update, cx);
        });
        Self {
            panel: Panel::new(tree, state.visible, state.view()),
            vault,
            workspace,
            focus_handle: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(),
            menu: None,
            _changes: changes,
        }
    }

    /// La vista que se ve, si el panel está visible.
    #[must_use]
    pub fn shown(&self) -> Option<PanelView> {
        self.panel.visible.then_some(self.panel.view)
    }

    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.panel.visible
    }

    #[must_use]
    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus_handle
    }

    /// Lo que se guarda al cerrar.
    #[must_use]
    pub fn state(&self) -> PanelState {
        PanelState::new(
            self.panel.visible,
            self.panel.view,
            self.panel.tree.expanded(),
        )
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        self.vault.root()
    }

    /// Selecciona en el árbol el archivo `path`, si está en el vault.
    pub fn reveal(&mut self, path: &Path, cx: &mut Context<Self>) {
        let Some(relative) = self.vault_path(path) else {
            return;
        };
        self.panel.tree.reveal(relative);
        self.scroll_to_selection();
        cx.notify();
    }

    /// Muestra `view`, o esconde el panel si ya la mostraba. Devuelve si
    /// quedó visible.
    pub fn toggle(&mut self, view: PanelView, cx: &mut Context<Self>) -> bool {
        self.menu = None;
        let visible = self.panel.toggle(view);
        cx.notify();
        visible
    }

    fn vault_path(&self, path: &Path) -> Option<VaultPath> {
        let resolved = path.canonicalize().ok()?;
        let relative = resolved.strip_prefix(self.vault.root()).ok()?;
        VaultPath::new(relative.to_str()?).ok()
    }

    fn apply(&mut self, update: &VaultUpdate, cx: &mut Context<Self>) {
        self.panel
            .tree
            .apply(&update.changes, Arc::clone(&update.snapshot));
        self.panel.follow(&update.changes);
        let selected = self.panel.tree.selected();
        if let Some(menu) = &mut self.menu
            && !menu.still_on(&update.changes, selected)
        {
            self.menu = None;
        }
        self.scroll_to_selection();
        cx.notify();
    }

    fn scroll_to_selection(&self) {
        if let Some(row) = self.panel.tree.selected_row() {
            self.scroll.scroll_to_item(row, ScrollStrategy::Nearest);
        }
    }

    /// Abre `path` donde dice `placement`; si el panel de edición enfocado
    /// lo muestra, la ventana le pasa el foco.
    fn open(&mut self, path: &VaultPath, placement: Placement, cx: &mut Context<Self>) {
        let file = shown_path(&self.vault.root().join(path.as_str()));
        let opened = self.workspace.update(cx, |workspace, cx| {
            cx.notify();
            workspace
                .open_file(&file, placement)
                .unwrap_or_else(|error| {
                    workspace.report(&error);
                    false
                })
        });
        if opened {
            cx.emit(SidebarEvent::Opened);
        }
    }

    fn file_of(&self, path: &VaultPath) -> std::path::PathBuf {
        self.vault.root().join(path.as_str())
    }

    fn has_unsaved_within(&self, path: &VaultPath, cx: &App) -> bool {
        self.workspace
            .read(cx)
            .has_unsaved_within(&self.file_of(path))
    }
}
