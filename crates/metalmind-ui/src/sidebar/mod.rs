//! El panel lateral: el árbol de carpetas y el de tags del vault, que se
//! actualizan con cada cambio que trae la [`VaultSession`] de la ventana, y las
//! operaciones de archivo. La lógica vive en
//! [`Panel`](crate::panel::Panel) y en el [`Workspace`]; aquí se pinta, se
//! traduce la entrada y se ejecuta lo que el usuario confirmó.

mod backlink_rows;
mod handlers;
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
use crate::finder::{FocusChanged, VaultChanged, VaultSession};
use crate::panel::{Outline, Panel, PanelView};
use crate::session::PanelState;
use crate::tag_change::TagRequest;
use crate::tag_outline::TagOutline;
use crate::workspace::{LinePosition, Placement, Workspace, shown_path};

/// Lo que la ventana tiene que hacer por el panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SidebarEvent {
    /// Se abrió un archivo en el panel de edición enfocado; con
    /// `focus_editor`, el foco pasa ahí y si no se queda en el árbol.
    Opened { focus_editor: bool },
    /// El foco vuelve al panel de edición.
    FocusEditor,
    /// Se cerraron paneles de edición (al borrar lo que mostraban); el
    /// foco se queda en el panel lateral.
    PanesChanged,
    /// Renombrar o fusionar un tag: la ventana arma el plan y muestra su
    /// vista previa.
    ChangeTags(TagRequest),
    /// Renombrar o mover una entrada: la ventana mira qué links cambian y,
    /// si alguno, muestra la vista previa antes de aplicar.
    Rename { from: VaultPath, to: VaultPath },
    /// Una nota nueva con este tag: la ventana pregunta su nombre y usa la
    /// plantilla que pida su página.
    NewTaggedNote(String),
    /// Abrir la página de este tag.
    OpenTagPage(String),
    /// Crear esta nota y abrirla: la ventana le pone los tags del focus y
    /// la plantilla que pidan.
    CreateNote(VaultPath),
}

pub struct Sidebar {
    panel: Panel,
    vault: Rc<Vault>,
    /// De donde sale el focus, que las dos vistas siguen.
    session: Entity<VaultSession>,
    workspace: Entity<Workspace>,
    focus_handle: FocusHandle,
    /// Cada vista con su scroll: al volver a una, sigue donde se dejó.
    scroll: UniformListScrollHandle,
    tag_scroll: UniformListScrollHandle,
    backlink_scroll: UniformListScrollHandle,
    menu: Option<ContextMenu>,
    /// Se abrió una nota desde los backlinks sin dejar el panel: la lista
    /// se queda en su nota hasta que el foco sale del panel, para poder
    /// recorrerla abriendo cada una.
    holding_backlinks: bool,
    _changes: Subscription,
    _focus: Subscription,
    _workspace: Subscription,
    _focus_out: Subscription,
}

impl EventEmitter<SidebarEvent> for Sidebar {}

impl Sidebar {
    /// El panel del vault de `session` como lo dejó `state`.
    pub fn new(
        session: &Entity<VaultSession>,
        state: &PanelState,
        workspace: Entity<Workspace>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let vault = session.read(cx).vault();
        let tree = FileTree::with_expanded(vault.snapshot(), state.expanded());
        let tags = TagOutline::new(session.read(cx).tags());
        let changes = cx.subscribe(session, |sidebar, _, VaultChanged(update), cx| {
            sidebar.apply(update, cx);
        });
        let focus = cx.subscribe(session, |sidebar, _, _: &FocusChanged, cx| {
            sidebar.menu = None;
            sidebar.sync_focus(cx);
            sidebar.scroll_to_selection();
            cx.notify();
        });
        // La nota activa se marca en el árbol y sus backlinks se muestran:
        // cambia con el panel de edición.
        let workspace_changes = cx.observe(&workspace, |sidebar, _, cx| {
            if !sidebar.holding_backlinks {
                sidebar.follow_active(cx);
            }
            cx.notify();
        });
        let focus_handle = cx.focus_handle();
        let focus_out = cx.on_focus_out(&focus_handle, window, |sidebar, _, _, cx| {
            sidebar.holding_backlinks = false;
            sidebar.follow_active(cx);
            cx.notify();
        });
        let mut panel = Panel::new(tree, tags, state.visible, state.view());
        panel.backlinks.set_index(vault.links());
        let mut sidebar = Self {
            panel,
            vault,
            session: session.clone(),
            workspace,
            focus_handle,
            scroll: UniformListScrollHandle::new(),
            tag_scroll: UniformListScrollHandle::new(),
            backlink_scroll: UniformListScrollHandle::new(),
            menu: None,
            holding_backlinks: false,
            _changes: changes,
            _focus: focus,
            _workspace: workspace_changes,
            _focus_out: focus_out,
        };
        sidebar.follow_active(cx);
        sidebar
    }

    /// Los backlinks son los de la nota del panel de edición enfocado.
    fn follow_active(&mut self, cx: &App) {
        let active = self.active(cx);
        self.panel.backlinks.show_note(active);
        self.refresh_backlinks();
    }

    fn refresh_backlinks(&mut self) {
        let before = self.panel.backlinks.selected_row();
        self.panel.backlinks.refresh();
        let after = self.panel.backlinks.selected_row();
        if let Some(row) = after.filter(|_| after != before) {
            self.backlink_scroll
                .scroll_to_item(row, ScrollStrategy::Nearest);
        }
    }

    /// Otra vista: la lista de backlinks deja de quedarse en su nota.
    fn release_backlinks(&mut self, cx: &App) {
        if self.holding_backlinks && self.panel.view != PanelView::Backlinks {
            self.holding_backlinks = false;
            self.follow_active(cx);
        }
    }

    /// Abre o cierra la nota de la fila `at` de los backlinks, sin abrirla.
    fn toggle_backlink_note(&mut self, at: usize, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus_handle, cx);
        self.panel.backlinks.select_row(at);
        self.panel.backlinks.toggle_selected();
        cx.notify();
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

    /// Lo que salió mal con un pedido del panel, en el panel.
    pub fn report(&mut self, error: &impl std::fmt::Display, cx: &mut Context<Self>) {
        self.panel.report(error);
        cx.notify();
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

    /// Selecciona en el árbol `path` del vault, ya o cuando llegue en un
    /// lote del watcher, como lo recién renombrado.
    pub fn reveal_entry(&mut self, path: VaultPath, cx: &mut Context<Self>) {
        self.panel.tree.reveal(path);
        self.scroll_to_selection();
        cx.notify();
    }

    /// Muestra `view`, o esconde el panel si ya la mostraba con el foco.
    /// Devuelve si quedó visible.
    pub fn toggle(&mut self, view: PanelView, window: &Window, cx: &mut Context<Self>) -> bool {
        self.menu = None;
        let focused = self.focus_handle.contains_focused(window, cx);
        let shown = self.panel.toggle(view, focused);
        self.release_backlinks(cx);
        self.reveal_tag_row(shown.moved);
        cx.notify();
        shown.visible
    }

    /// Lleva a la vista la fila de tags que rehacer la vista movió; si no se
    /// movió, la vista queda donde se dejó.
    fn reveal_tag_row(&self, moved: Option<usize>) {
        if let Some(row) = moved {
            self.tag_scroll.scroll_to_item(row, ScrollStrategy::Nearest);
        }
    }

    fn vault_path(&self, path: &Path) -> Option<VaultPath> {
        let resolved = path.canonicalize().ok()?;
        let relative = resolved.strip_prefix(self.vault.root()).ok()?;
        VaultPath::new(relative.to_str()?).ok()
    }

    fn apply(&mut self, update: &VaultUpdate, cx: &mut Context<Self>) {
        let in_scope = self.session.read(cx).in_scope();
        self.panel
            .tree
            .apply(&update.changes, Arc::clone(&update.snapshot), in_scope);
        self.panel.follow(&update.changes);
        self.panel.set_tags(Arc::clone(&update.tags));
        self.sync_focus(cx);
        self.panel.backlinks.follow(&update.changes);
        self.panel.backlinks.set_index(Arc::clone(&update.links));
        let chosen_backlink = self.panel.backlinks.selected_id().cloned();
        self.refresh_backlinks();
        let still_chosen = self.panel.backlinks.selected_id() == chosen_backlink.as_ref();
        let selected = self.panel.outline().selected_path();
        let selected_tag = self.panel.selected_tag();
        // El menú de un backlink abre en su link: si el lote cambió de fila
        // elegida, abriría otro.
        let backlink_moved = self.panel.view == PanelView::Backlinks && !still_chosen;
        if let Some(menu) = &mut self.menu
            && (backlink_moved
                || !menu.still_on(&update.changes, selected, selected_tag.as_deref()))
        {
            self.menu = None;
        }
        self.scroll_to_selection();
        cx.notify();
    }

    /// Las vistas con el focus de la sesión y lo que deja ver ahora.
    fn sync_focus(&mut self, cx: &App) {
        let session = self.session.read(cx);
        let scope = session
            .focus()
            .map(|focus| focus.scope().clone())
            .unwrap_or_default();
        self.panel.set_focus(scope, session.in_scope());
    }

    /// Cada vista lleva su scroll a su selección, se vea o no: lo que
    /// `reveal` o el watcher seleccionan en la escondida se ve al volver.
    fn scroll_to_selection(&self) {
        let views: [(&dyn Outline, &UniformListScrollHandle); 3] = [
            (&self.panel.tree, &self.scroll),
            (&self.panel.tags, &self.tag_scroll),
            (&self.panel.backlinks, &self.backlink_scroll),
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

    /// Abre `path` donde dice `placement`, en `at` si se da; si el panel de
    /// edición enfocado lo muestra y `focus_editor`, la ventana le pasa el
    /// foco.
    fn open(
        &mut self,
        path: &VaultPath,
        at: Option<LinePosition>,
        placement: Placement,
        focus_editor: bool,
        cx: &mut Context<Self>,
    ) {
        let file = shown_path(&self.vault.root().join(path.as_str()));
        let opened = self.workspace.update(cx, |workspace, cx| {
            cx.notify();
            match at {
                Some(at) => workspace.open_file_at(&file, placement, at),
                None => workspace.open_file(&file, placement),
            }
            .unwrap_or_else(|error| {
                workspace.report(&error);
                false
            })
        });
        if !opened {
            return;
        }
        // Abierta desde los backlinks sin pasarle el foco, la lista se queda
        // en su nota. Basta marcarlo aquí: lo que avisa el panel de edición
        // llega al observador cuando termina esta actualización. Si no se
        // abrió nada, la lista sigue a la nota activa.
        if self.panel.view == PanelView::Backlinks && !focus_editor {
            self.holding_backlinks = true;
        }
        cx.emit(SidebarEvent::Opened { focus_editor });
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
