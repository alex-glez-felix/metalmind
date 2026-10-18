//! El panel lateral: el árbol de carpetas del vault, que se actualiza con
//! el watcher, y las operaciones de archivo. La lógica vive en
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

use std::io;
use std::path::Path;
use std::sync::{Arc, mpsc};
use std::thread;

use gpui_kit::{
    App, Context, Entity, EventEmitter, FocusHandle, ScrollStrategy, Task, UniformListScrollHandle,
};
use nota_vault::{Vault, VaultChange, VaultPath, VaultUpdate};

pub use self::actions::bind_keys;
use self::menu::ContextMenu;
pub use self::rail::rail;
use crate::file_tree::FileTree;
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
    vault: Vault,
    workspace: Entity<Workspace>,
    focus_handle: FocusHandle,
    scroll: UniformListScrollHandle,
    menu: Option<ContextMenu>,
    _updates: Task<()>,
}

impl EventEmitter<SidebarEvent> for Sidebar {}

impl Sidebar {
    /// El panel de `vault` como lo dejó `state`; `updates` sale de
    /// [`bridge`].
    pub fn new(
        vault: Vault,
        updates: async_channel::Receiver<VaultUpdate>,
        state: &PanelState,
        workspace: Entity<Workspace>,
        cx: &mut Context<Self>,
    ) -> Self {
        let tree = FileTree::with_expanded(vault.snapshot(), state.expanded());
        Self {
            panel: Panel::new(tree, state.visible, state.view()),
            vault,
            workspace,
            focus_handle: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(),
            menu: None,
            _updates: watch(updates, cx),
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

    fn apply(&mut self, updates: Vec<VaultUpdate>, cx: &mut Context<Self>) {
        let Some(snapshot) = updates.last().map(|update| Arc::clone(&update.snapshot)) else {
            return;
        };
        let changes: Vec<VaultChange> = updates.into_iter().flat_map(|u| u.changes).collect();
        self.panel.tree.apply(&changes, snapshot);
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

/// Pasa los cambios del watcher, que llegan por un `mpsc` bloqueante, a
/// un canal que el bucle de GPUI puede esperar. El hilo termina cuando el
/// vault suelta su watcher o cuando ya nadie escucha.
///
/// # Errors
/// Si no se puede arrancar el hilo.
pub fn bridge(
    updates: mpsc::Receiver<VaultUpdate>,
) -> io::Result<async_channel::Receiver<VaultUpdate>> {
    let (sender, receiver) = async_channel::unbounded();
    thread::Builder::new()
        .name("nota-ui vault".to_owned())
        .spawn(move || {
            for update in updates {
                if sender.send_blocking(update).is_err() {
                    break;
                }
            }
        })?;
    Ok(receiver)
}

/// Aplica los cambios a medida que llegan; lo que se junte mientras la UI
/// está ocupada se aplica de una vez.
fn watch(updates: async_channel::Receiver<VaultUpdate>, cx: &mut Context<Sidebar>) -> Task<()> {
    cx.spawn(async move |sidebar, cx| {
        while let Ok(first) = updates.recv().await {
            let mut batch = vec![first];
            while let Ok(more) = updates.try_recv() {
                batch.push(more);
            }
            if sidebar
                .update(cx, |sidebar, cx| sidebar.apply(batch, cx))
                .is_err()
            {
                break;
            }
        }
    })
}
