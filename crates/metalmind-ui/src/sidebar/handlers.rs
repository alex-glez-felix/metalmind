//! Qué hace cada acción del panel.

use gpui_kit::{Context, Window};
use metalmind_vault::EntryKind;

use super::actions::{
    Cancel, CollapseOrParent, Confirm, Delete, ExpandOrOpen, Filter, Hide, MenuNext, MenuPrevious,
    Move, NewFolder, NewNote, Open, OpenSplitDown, OpenSplitRight, Rename, SelectFirst, SelectLast,
    SelectNext, SelectPrevious, ToggleView,
};
use super::{Sidebar, SidebarEvent};
use crate::file_tree::FileTree;
use crate::panel::{NewEntry, Request};
use crate::workspace::{Axis, Placement};

impl Sidebar {
    /// Una operación del árbol que mueve la selección.
    fn navigate(&mut self, cx: &mut Context<Self>, step: impl FnOnce(&mut FileTree)) {
        step(&mut self.panel.tree);
        self.scroll_to_selection();
        cx.notify();
    }

    pub(super) fn select_next(&mut self, _: &SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        self.navigate(cx, FileTree::select_next);
    }

    pub(super) fn select_previous(
        &mut self,
        _: &SelectPrevious,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.navigate(cx, FileTree::select_previous);
    }

    pub(super) fn select_first(&mut self, _: &SelectFirst, _: &mut Window, cx: &mut Context<Self>) {
        self.navigate(cx, FileTree::select_first);
    }

    pub(super) fn select_last(&mut self, _: &SelectLast, _: &mut Window, cx: &mut Context<Self>) {
        self.navigate(cx, FileTree::select_last);
    }

    pub(super) fn collapse_or_parent(
        &mut self,
        _: &CollapseOrParent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.navigate(cx, FileTree::collapse_or_parent);
    }

    pub(super) fn expand_or_open(
        &mut self,
        _: &ExpandOrOpen,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.panel.tree.selected_entry() {
            Some(row) if row.kind != EntryKind::Folder => {
                let path = row.path.clone();
                self.open(&path, Placement::Current, cx);
            }
            _ => self.navigate(cx, FileTree::expand_or_enter),
        }
    }

    /// Un archivo se abre; una carpeta se abre o se cierra.
    pub(super) fn open_selected(&mut self, _: &Open, _: &mut Window, cx: &mut Context<Self>) {
        let Some(row) = self.panel.tree.selected_entry() else {
            return;
        };
        let path = row.path.clone();
        if row.kind == EntryKind::Folder {
            self.navigate(cx, |tree| tree.toggle(&path));
        } else {
            self.open(&path, Placement::Current, cx);
        }
    }

    pub(super) fn open_split_right(
        &mut self,
        _: &OpenSplitRight,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_selected_in(Placement::Split(Axis::Row), cx);
    }

    pub(super) fn open_split_down(
        &mut self,
        _: &OpenSplitDown,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_selected_in(Placement::Split(Axis::Column), cx);
    }

    fn open_selected_in(&mut self, placement: Placement, cx: &mut Context<Self>) {
        self.menu = None;
        let Some(row) = self.panel.tree.selected_entry() else {
            return;
        };
        if row.kind != EntryKind::Folder {
            let path = row.path.clone();
            self.open(&path, placement, cx);
        }
    }

    pub(super) fn filter(&mut self, _: &Filter, _: &mut Window, cx: &mut Context<Self>) {
        self.panel.start_filter();
        cx.notify();
    }

    pub(super) fn new_note(&mut self, _: &NewNote, _: &mut Window, cx: &mut Context<Self>) {
        self.menu = None;
        self.panel.start_create(NewEntry::Note);
        cx.notify();
    }

    pub(super) fn new_folder(&mut self, _: &NewFolder, _: &mut Window, cx: &mut Context<Self>) {
        self.menu = None;
        self.panel.start_create(NewEntry::Folder);
        cx.notify();
    }

    pub(super) fn rename(&mut self, _: &Rename, _: &mut Window, cx: &mut Context<Self>) {
        self.menu = None;
        self.panel.start_rename();
        cx.notify();
    }

    pub(super) fn move_entry(&mut self, _: &Move, _: &mut Window, cx: &mut Context<Self>) {
        self.menu = None;
        self.panel.start_move();
        cx.notify();
    }

    pub(super) fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        self.menu = None;
        let Some(path) = self.panel.tree.selected().cloned() else {
            return;
        };
        let unsaved = self.has_unsaved_within(&path, cx);
        self.panel.start_delete(path, unsaved);
        cx.notify();
    }

    pub(super) fn confirm(&mut self, _: &Confirm, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(menu) = self.menu.take() {
            if let Some(item) = menu.selected_item() {
                window.dispatch_action(item.action(), cx);
            }
            cx.notify();
            return;
        }
        if let Some(request) = self.panel.confirm() {
            self.execute(request, cx);
        }
        cx.notify();
    }

    pub(super) fn cancel(&mut self, _: &Cancel, _: &mut Window, cx: &mut Context<Self>) {
        if self.menu.take().is_none() {
            if self.panel.prompt().is_some() {
                self.panel.cancel();
            } else {
                self.panel.tree.set_filter("");
            }
        }
        self.scroll_to_selection();
        cx.notify();
    }

    pub(super) fn menu_next(&mut self, _: &MenuNext, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(menu) = &mut self.menu {
            menu.select_next();
        }
        cx.notify();
    }

    pub(super) fn menu_previous(
        &mut self,
        _: &MenuPrevious,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = &mut self.menu {
            menu.select_previous();
        }
        cx.notify();
    }

    pub(super) fn toggle_view(&mut self, _: &ToggleView, _: &mut Window, cx: &mut Context<Self>) {
        self.panel.view = self.panel.view.other();
        cx.notify();
    }

    pub(super) fn hide(&mut self, _: &Hide, _: &mut Window, cx: &mut Context<Self>) {
        self.panel.visible = false;
        self.menu = None;
        cx.emit(SidebarEvent::FocusEditor);
        cx.notify();
    }

    /// Lo que el usuario confirmó. El árbol se entera por el watcher; lo
    /// creado o renombrado queda seleccionado cuando llega, y una nota
    /// nueva se abre para escribirla.
    fn execute(&mut self, request: Request, cx: &mut Context<Self>) {
        let result = match request {
            Request::CreateNote(path) => self.vault.create_note(&path).map(|()| {
                self.panel.tree.reveal(path.clone());
                self.open(&path, Placement::Current, cx);
            }),
            Request::CreateFolder(path) => self
                .vault
                .create_folder(&path)
                .map(|()| self.panel.tree.reveal(path)),
            Request::Rename { from, to } => {
                let workspace = self.workspace.clone();
                let renamed = workspace.update(cx, |workspace, cx| {
                    cx.notify();
                    workspace.rename_file(&self.vault, &from, &to)
                });
                renamed.map(|()| self.panel.tree.reveal(to))
            }
            Request::Trash(path) => {
                let workspace = self.workspace.clone();
                let trashed = workspace.update(cx, |workspace, cx| {
                    cx.notify();
                    workspace.trash_file(&self.vault, &path)
                });
                trashed.map(|()| cx.emit(SidebarEvent::PanesChanged))
            }
        };
        if let Err(error) = result {
            log::error!("{error}");
            self.panel.report(&error);
        }
        self.scroll_to_selection();
    }
}
