//! El panel de contexto en la ventana (#164): a la derecha de los paneles y
//! solo mientras se pide (`space b`), con los links que llegan a la nota del
//! panel de edición enfocado y los que salen de ella. La lógica vive en
//! [`NoteContext`]; aquí se sigue a la nota activa y al watcher, y se abre lo
//! elegido.

mod handlers;
mod render;
mod rows;

use std::path::PathBuf;

use gpui_kit::{
    App, Context, Entity, EventEmitter, FocusHandle, ScrollStrategy, Subscription,
    UniformListScrollHandle, Window,
};
use metalmind_vault::VaultPath;

use crate::finder::{VaultChanged, VaultSession};
use crate::note_context::{ContextTab, NoteContext};
use crate::panel::{ClickOpen, Outline as _};
use crate::workspace::{Placement, Workspace, shown_path};

/// Lo que la ventana tiene que hacer por el panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContextEvent {
    /// Se abrió una nota en el panel de edición enfocado; con
    /// `focus_editor`, el foco pasa ahí.
    Opened { focus_editor: bool },
    /// El foco vuelve al panel de edición, también al cerrarse el panel.
    FocusEditor,
}

pub struct ContextPanel {
    context: NoteContext,
    visible: bool,
    root: PathBuf,
    session: Entity<VaultSession>,
    workspace: Entity<Workspace>,
    focus_handle: FocusHandle,
    /// Cada pestaña con su scroll.
    backlink_scroll: UniformListScrollHandle,
    outgoing_scroll: UniformListScrollHandle,
    /// Se abrió una nota desde el panel sin dejarlo: se queda en su nota
    /// hasta que el foco sale, para poder recorrerlo abriendo cada una.
    holding: bool,
    _changes: Subscription,
    _workspace: Subscription,
    _focus_out: Subscription,
}

impl EventEmitter<ContextEvent> for ContextPanel {}

impl ContextPanel {
    /// Cerrado: aparece cuando se pide.
    pub fn new(
        session: &Entity<VaultSession>,
        workspace: Entity<Workspace>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let vault = session.read(cx).vault();
        let changes = cx.subscribe(session, |panel, _, VaultChanged(update), cx| {
            panel.context.apply(&update.changes, &update.links);
            panel.scroll_to_selection();
            cx.notify();
        });
        let workspace_changes = cx.observe(&workspace, |panel, _, cx| {
            if panel.visible && !panel.holding {
                panel.follow_active(cx);
            }
        });
        let focus_handle = cx.focus_handle();
        let focus_out = cx.on_focus_out(&focus_handle, window, |panel, _, _, cx| {
            if std::mem::take(&mut panel.holding) {
                panel.follow_active(cx);
            }
        });
        Self {
            context: NoteContext::new(vault.links()),
            visible: false,
            root: vault.root().to_owned(),
            session: session.clone(),
            workspace,
            focus_handle,
            backlink_scroll: UniformListScrollHandle::new(),
            outgoing_scroll: UniformListScrollHandle::new(),
            holding: false,
            _changes: changes,
            _workspace: workspace_changes,
            _focus_out: focus_out,
        }
    }

    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    #[must_use]
    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus_handle
    }

    /// Lo muestra con la nota activa, o lo cierra si ya se veía con el foco:
    /// sin el foco, sigue visible para que lo tome. Devuelve si quedó
    /// visible.
    pub fn toggle(&mut self, window: &Window, cx: &mut Context<Self>) -> bool {
        let focused = self.focus_handle.contains_focused(window, cx);
        self.visible = !(self.visible && focused);
        if self.visible {
            self.follow_active(cx);
        } else {
            self.holding = false;
        }
        cx.notify();
        self.visible
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        self.visible = false;
        self.holding = false;
        cx.emit(ContextEvent::FocusEditor);
        cx.notify();
    }

    /// El contexto es el de la nota del panel de edición enfocado.
    fn follow_active(&mut self, cx: &mut Context<Self>) {
        let active = self.active(cx);
        if self.context.note() != active.as_ref() {
            self.context.show_note(active);
            self.scroll_to_selection();
        }
        cx.notify();
    }

    fn active(&self, cx: &App) -> Option<VaultPath> {
        let document = self.workspace.read(cx).focused_document().ok()?;
        self.session.read(cx).vault_path(document.path()?)
    }

    fn set_tab(&mut self, tab: ContextTab, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus_handle, cx);
        self.context.set_tab(tab);
        cx.notify();
    }

    fn scroll(&self) -> &UniformListScrollHandle {
        match self.context.tab() {
            ContextTab::Backlinks => &self.backlink_scroll,
            ContextTab::Outgoing => &self.outgoing_scroll,
        }
    }

    fn scroll_to_selection(&self) {
        if let Some(row) = self.context.outline().selected_row() {
            self.scroll().scroll_to_item(row, ScrollStrategy::Nearest);
        }
    }

    /// Un clic en la fila `at`: la abre y, con doble clic, le pasa el foco
    /// al editor.
    fn click(&mut self, at: usize, clicks: usize, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus_handle, cx);
        if let Some(open) = self.context.click(at, clicks) {
            self.open(&open, Placement::Current, cx);
        }
        cx.notify();
    }

    /// Abre o cierra la nota de la fila `at` de los backlinks, sin abrirla.
    fn toggle_note(&mut self, at: usize, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus_handle, cx);
        let outline = &mut self.context.backlinks;
        outline.select_row(at);
        outline.toggle_selected();
        cx.notify();
    }

    fn open(&mut self, open: &ClickOpen, placement: Placement, cx: &mut Context<Self>) {
        let file = shown_path(&self.root.join(open.path.as_str()));
        let opened = self.workspace.update(cx, |workspace, cx| {
            cx.notify();
            match open.at {
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
        // Basta marcarlo aquí: lo que avisa el panel de edición llega al
        // observador cuando termina esta actualización.
        self.holding = !open.focus_editor;
        cx.emit(ContextEvent::Opened {
            focus_editor: open.focus_editor,
        });
    }
}

#[cfg(test)]
mod tests;
