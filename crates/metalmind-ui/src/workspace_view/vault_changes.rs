//! Lo que [`VaultChanges`](crate::vault_changes::VaultChanges) le pide a la ventana: mostrar y cerrar su vista
//! previa, abrir una línea y seguir a lo que renombró.

use gpui_kit::{Context, Window};

use super::WorkspaceView;
use super::overlay::Overlay;
use crate::picker::Open;
use crate::vault_changes::VaultChangesEvent;

impl WorkspaceView {
    pub(super) fn on_vault_changes_event(
        &mut self,
        event: &VaultChangesEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            VaultChangesEvent::ShowPreview(view) => {
                let view = view.clone();
                self.show(Overlay::ChangePreview { view }, window, cx);
            }
            VaultChangesEvent::ClosePreview { focus_sidebar } => {
                self.close_overlay(window, cx);
                if *focus_sidebar {
                    self.focus_sidebar(window, cx);
                }
            }
            VaultChangesEvent::OpenLine { path, at } => {
                self.open_note_at(path, *at, Open::Here, window, cx);
            }
            VaultChangesEvent::RenamePlanned => {
                let overlay_open = self.overlay.is_some();
                self.vault_changes
                    .update(cx, |changes, cx| changes.finish_rename(overlay_open, cx));
            }
            VaultChangesEvent::Renamed(to) => {
                self.sidebar
                    .update(cx, |sidebar, cx| sidebar.reveal_entry(to.clone(), cx));
                self.sync_editors(cx);
            }
            VaultChangesEvent::SavedViewsChanged => {
                self.vault_views(cx);
            }
        }
    }
}
