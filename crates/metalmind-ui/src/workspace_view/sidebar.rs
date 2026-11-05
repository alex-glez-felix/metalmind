//! El panel lateral en la ventana: mostrar una vista o esconderlo, darle el
//! foco y hacer lo que pide, que necesita el vault o los editores.

use std::path::Path;

use gpui_kit::{Context, Window};

use super::WorkspaceView;
use crate::panel::PanelView;
use crate::sidebar::SidebarEvent;

impl WorkspaceView {
    /// Selecciona `file` en el árbol del panel lateral.
    pub fn reveal(&self, file: &Path, cx: &mut Context<Self>) {
        self.sidebar
            .update(cx, |sidebar, cx| sidebar.reveal(file, cx));
    }

    #[must_use]
    pub fn sidebar_visible(&self, cx: &Context<Self>) -> bool {
        self.sidebar.read(cx).is_visible()
    }

    pub fn focus_sidebar(&self, window: &mut Window, cx: &mut Context<Self>) {
        let handle = self.sidebar.read(cx).focus_handle().clone();
        window.focus(&handle, cx);
    }

    pub(super) fn toggle_panel(
        &mut self,
        view: PanelView,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let visible = self
            .sidebar
            .update(cx, |sidebar, cx| sidebar.toggle(view, window, cx));
        if visible {
            self.focus_sidebar(window, cx);
        } else {
            self.focus(window, cx);
        }
        cx.notify();
    }

    pub(super) fn on_sidebar_event(
        &mut self,
        event: SidebarEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            SidebarEvent::Opened { focus_editor } => {
                self.sync_editors(cx);
                if focus_editor {
                    self.focus(window, cx);
                }
                self.touch_focused(cx);
            }
            SidebarEvent::PanesChanged => self.sync_editors(cx),
            // A la derecha del panel está la lista, si se ve.
            SidebarEvent::FocusEditor if self.list.read(cx).is_visible() => {
                self.focus_list(window, cx);
            }
            SidebarEvent::FocusEditor => self.focus(window, cx),
            SidebarEvent::ChangeTags(request) => {
                self.vault_changes
                    .update(cx, |changes, cx| changes.change_tags(&request, cx));
            }
            SidebarEvent::Rename { from, to } => {
                self.vault_changes
                    .update(cx, |changes, cx| changes.rename(&from, &to, cx));
            }
            SidebarEvent::NewTaggedNote(tag) => self.new_tagged_note(tag, window, cx),
            SidebarEvent::OpenTagPage(tag) => self.open_tag_page(&tag, window, cx),
            SidebarEvent::CreateNote(path) => self.create_panel_note(&path, window, cx),
            SidebarEvent::ShowView { view, focus_list } => {
                self.show_view(&view, focus_list, window, cx);
            }
        }
        cx.notify();
    }
}
