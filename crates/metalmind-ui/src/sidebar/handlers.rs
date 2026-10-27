//! Qué hace cada acción del panel.

use gpui_kit::{Context, Window};

use metalmind_keymap::{Action, PanelAction};

use super::{Sidebar, SidebarEvent};
use crate::actions::Dispatch;
use crate::panel::{NewEntry, Outline, PanelView, Request};
use crate::tag_change::TagField;
use crate::workspace::{Axis, Placement};

impl Sidebar {
    /// Las acciones del panel; las demás siguen hacia la raíz de la ventana.
    pub(super) fn on_dispatch(
        &mut self,
        Dispatch(invocation): &Dispatch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Action::Panel(action) = invocation.action else {
            cx.propagate();
            return;
        };
        match action {
            PanelAction::ToggleView => self.toggle_view(window, cx),
            PanelAction::Hide => self.hide(window, cx),
            PanelAction::FocusEditor => cx.emit(SidebarEvent::FocusEditor),
            PanelAction::SelectNext => self.select_next(window, cx),
            PanelAction::SelectPrevious => self.select_previous(window, cx),
            PanelAction::SelectFirst => self.select_first(window, cx),
            PanelAction::SelectLast => self.select_last(window, cx),
            PanelAction::CollapseOrParent => self.collapse_or_parent(window, cx),
            PanelAction::ExpandOrOpen => self.expand_or_open(window, cx),
            PanelAction::Open => self.open_selected(window, cx),
            PanelAction::OpenSplitRight => self.open_split_right(window, cx),
            PanelAction::OpenSplitDown => self.open_split_down(window, cx),
            PanelAction::Filter => self.filter(window, cx),
            PanelAction::NewNote => self.new_note(window, cx),
            PanelAction::NewFolder => self.new_folder(window, cx),
            PanelAction::Rename => self.rename(window, cx),
            PanelAction::Move => self.move_entry(window, cx),
            PanelAction::EditTagAliases => self.tag_field(TagField::Aliases, cx),
            PanelAction::EditTagParents => self.tag_field(TagField::Parents, cx),
            PanelAction::SetTagIcon => self.tag_field(TagField::Icon, cx),
            PanelAction::SetTagTemplate => self.tag_field(TagField::Template, cx),
            PanelAction::OpenTagPage => self.open_tag_page(cx),
            PanelAction::ToggleFocus => self.toggle_focus(cx),
            PanelAction::Delete => self.delete(window, cx),
            PanelAction::Confirm => self.confirm(window, cx),
            PanelAction::Cancel => self.cancel(window, cx),
            PanelAction::MenuNext => self.menu_next(window, cx),
            PanelAction::MenuPrevious => self.menu_previous(window, cx),
        }
    }

    /// Una operación de la vista que se ve que mueve la selección.
    fn navigate(&mut self, cx: &mut Context<Self>, step: impl FnOnce(&mut dyn Outline)) {
        step(self.panel.outline_mut());
        self.scroll_to_selection();
        cx.notify();
    }

    fn select_next(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.navigate(cx, |outline| outline.select_next());
    }

    fn select_previous(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.navigate(cx, |outline| outline.select_previous());
    }

    fn select_first(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.navigate(cx, |outline| outline.select_first());
    }

    fn select_last(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.navigate(cx, |outline| outline.select_last());
    }

    fn collapse_or_parent(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.navigate(cx, |outline| outline.collapse_or_parent());
    }

    /// En los backlinks, `l` en una nota la abre o baja a sus links, como
    /// en una carpeta, y en un link lo abre.
    fn expand_or_open(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let is_backlink_note = self.panel.view == PanelView::Backlinks
            && matches!(
                self.panel.backlinks.selected_entry(),
                Some(crate::backlinks::Row::Note { .. })
            );
        if !is_backlink_note && self.open_selected_file(Placement::Current, cx) {
            return;
        }
        self.navigate(cx, |outline| outline.expand_or_enter());
    }

    /// Un archivo se abre; una carpeta o un tag se abre o se cierra.
    fn open_selected(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if !self.open_selected_file(Placement::Current, cx) {
            self.navigate(cx, |outline| outline.toggle_selected());
        }
    }

    /// Abre el archivo de la fila elegida, donde lo dice ella; si no hay,
    /// devuelve `false`.
    fn open_selected_file(&mut self, placement: Placement, cx: &mut Context<Self>) -> bool {
        let outline = self.panel.outline();
        let Some(path) = outline.selected_file().cloned() else {
            return false;
        };
        let at = outline.selected_position();
        self.open(&path, at, placement, true, cx);
        true
    }

    fn open_split_right(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.open_selected_in(Placement::Split(Axis::Row), cx);
    }

    fn open_split_down(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.open_selected_in(Placement::Split(Axis::Column), cx);
    }

    fn open_selected_in(&mut self, placement: Placement, cx: &mut Context<Self>) {
        self.menu = None;
        self.open_selected_file(placement, cx);
    }

    fn filter(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.panel.start_filter();
        cx.notify();
    }

    /// En la vista de tags, con el tag elegido.
    fn new_note(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.menu = None;
        if self.panel.view == PanelView::Tags {
            if let Some(tag) = self.panel.selected_tag() {
                cx.emit(SidebarEvent::NewTaggedNote(tag));
            }
        } else {
            self.panel.start_create(NewEntry::Note);
        }
        cx.notify();
    }

    fn new_folder(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.menu = None;
        self.panel.start_create(NewEntry::Folder);
        cx.notify();
    }

    fn rename(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.menu = None;
        self.panel.start_rename();
        cx.notify();
    }

    fn move_entry(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.menu = None;
        self.panel.start_move();
        cx.notify();
    }

    fn tag_field(&mut self, field: TagField, cx: &mut Context<Self>) {
        self.menu = None;
        self.panel.start_tag_field(field);
        cx.notify();
    }

    fn open_tag_page(&mut self, cx: &mut Context<Self>) {
        self.menu = None;
        if self.panel.view == PanelView::Tags
            && let Some(tag) = self.panel.selected_tag()
        {
            cx.emit(SidebarEvent::OpenTagPage(tag));
        }
        cx.notify();
    }

    fn toggle_focus(&mut self, cx: &mut Context<Self>) {
        self.menu = None;
        if let Some(focus) = self.panel.selected_focus() {
            self.session
                .update(cx, |session, cx| session.toggle_focus(focus, cx));
        }
        cx.notify();
    }

    fn delete(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.menu = None;
        let Some(path) = self.panel.tree.selected().cloned() else {
            return;
        };
        let unsaved = self.has_unsaved_within(&path, cx);
        self.panel.start_delete(path, unsaved);
        cx.notify();
    }

    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(menu) = self.menu.take() {
            if let Some(item) = menu.selected_item() {
                window.dispatch_action(Box::new(Dispatch(item.action().into())), cx);
            }
            cx.notify();
            return;
        }
        if let Some(request) = self.panel.confirm() {
            self.execute(request, cx);
        }
        cx.notify();
    }

    fn cancel(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.menu.take().is_none() {
            self.panel.escape();
        }
        self.scroll_to_selection();
        cx.notify();
    }

    fn menu_next(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(menu) = &mut self.menu {
            menu.select_next();
        }
        cx.notify();
    }

    fn menu_previous(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(menu) = &mut self.menu {
            menu.select_previous();
        }
        cx.notify();
    }

    fn toggle_view(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.menu = None;
        let moved = self.panel.switch_view();
        self.reveal_tag_row(moved);
        cx.notify();
    }

    fn hide(&mut self, _: &mut Window, cx: &mut Context<Self>) {
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
            Request::CreateNote(path) => {
                self.panel.tree.reveal(path.clone());
                cx.emit(SidebarEvent::CreateNote(path));
                Ok(())
            }
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
            Request::ChangeTags(request) => {
                cx.emit(SidebarEvent::ChangeTags(request));
                Ok(())
            }
        };
        if let Err(error) = result {
            log::error!("{error}");
            self.panel.report(&error);
        }
        self.scroll_to_selection();
    }
}
