//! El panel lateral: el árbol de carpetas y el de tags del vault, que se
//! actualizan con cada cambio que trae la [`VaultSession`] de la ventana, y las
//! operaciones de archivo. La lógica vive en
//! [`Panel`](crate::panel::Panel) y en el [`Workspace`]; aquí se pinta, se
//! traduce la entrada y se ejecuta lo que el usuario confirmó.

mod handlers;
mod icons;
mod input;
mod menu;
mod prompt;
mod rail;
mod render;
mod tag_rows;

use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::{
    App, Context, Entity, EventEmitter, FocusHandle, ScrollStrategy, Subscription,
    UniformListScrollHandle, Window,
};
use metalmind_vault::{Vault, VaultPath, VaultUpdate};

use self::menu::ContextMenu;
pub use self::rail::rail;
use crate::file_tree::FileTree;
use crate::finder::{VaultChanged, VaultSession};
use crate::panel::{Outline, Panel, PanelView};
use crate::session::PanelState;
use crate::tag_outline::TagOutline;
use crate::workspace::{Placement, Workspace, shown_path};

/// Lo que la ventana tiene que hacer por el panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarEvent {
    /// Se abrió un archivo en el panel de edición enfocado; con
    /// `focus_editor`, el foco pasa ahí y si no se queda en el árbol.
    Opened { focus_editor: bool },
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
    /// Cada vista con su scroll: al volver a una, sigue donde se dejó.
    scroll: UniformListScrollHandle,
    tag_scroll: UniformListScrollHandle,
    menu: Option<ContextMenu>,
    _changes: Subscription,
    _workspace: Subscription,
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
        let tags = TagOutline::new(session.read(cx).tags());
        let changes = cx.subscribe(session, |sidebar, _, VaultChanged(update), cx| {
            sidebar.apply(update, cx);
        });
        // La nota activa se marca en el árbol: cambia con el panel de edición.
        let workspace_changes = cx.observe(&workspace, |_, _, cx| cx.notify());
        Self {
            panel: Panel::new(tree, tags, state.visible, state.view()),
            vault,
            workspace,
            focus_handle: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(),
            tag_scroll: UniformListScrollHandle::new(),
            menu: None,
            _changes: changes,
            _workspace: workspace_changes,
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

    /// Muestra `view`, o esconde el panel si ya la mostraba con el foco.
    /// Devuelve si quedó visible.
    pub fn toggle(&mut self, view: PanelView, window: &Window, cx: &mut Context<Self>) -> bool {
        self.menu = None;
        let focused = self.focus_handle.contains_focused(window, cx);
        let visible = self.panel.toggle(view, focused);
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
        self.panel.set_tags(Arc::clone(&update.tags));
        let selected = self.panel.outline().selected_path();
        if let Some(menu) = &mut self.menu
            && !menu.still_on(&update.changes, selected)
        {
            self.menu = None;
        }
        self.scroll_to_selection();
        cx.notify();
    }

    /// Cada vista lleva su scroll a su selección, se vea o no: lo que
    /// `reveal` o el watcher seleccionan en la escondida se ve al volver.
    fn scroll_to_selection(&self) {
        let views: [(&dyn Outline, &UniformListScrollHandle); 2] = [
            (&self.panel.tree, &self.scroll),
            (&self.panel.tags, &self.tag_scroll),
        ];
        for (outline, handle) in views {
            if let Some(row) = outline.selected_row() {
                handle.scroll_to_item(row, ScrollStrategy::Nearest);
            }
        }
    }

    /// La nota del panel de edición enfocado, si es del vault.
    fn active(&self, cx: &App) -> Option<VaultPath> {
        let workspace = self.workspace.read(cx);
        let document = workspace.focused_document().ok()?;
        self.vault_path(document.path()?)
    }

    /// Abre `path` donde dice `placement`; si el panel de edición enfocado
    /// lo muestra y `focus_editor`, la ventana le pasa el foco.
    fn open(
        &mut self,
        path: &VaultPath,
        placement: Placement,
        focus_editor: bool,
        cx: &mut Context<Self>,
    ) {
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
            cx.emit(SidebarEvent::Opened { focus_editor });
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
