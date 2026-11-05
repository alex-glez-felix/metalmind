//! Los comandos en la ventana: una acción que llega a la raíz se ejecuta en
//! el [`Workspace`](crate::workspace::Workspace), y lo que este pide hacer
//! fuera de él ([`Effect`]) lo hace cada submódulo de la ventana.

use gpui_kit::{Context, Window};
use metalmind_keymap::{Action, Invocation, WindowCommand};

use super::WorkspaceView;
use super::list_views::ListViewChange;
use crate::actions::Dispatch;
use crate::clipboard::SystemClipboard;
use crate::new_note::NoteRequest;
use crate::panel::PanelView;
use crate::vault_changes::VaultChanges;
use crate::workspace::{Direction, Effect};

impl WorkspaceView {
    /// Los comandos y las acciones del motor llegan aquí desde cualquier
    /// vista; lo demás llega solo si su vista no tenía el foco.
    pub(super) fn dispatch(
        &mut self,
        invocation: &Invocation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match invocation.action {
            Action::Command(_) | Action::Window(_) => self.run(invocation, window, cx),
            Action::Vim(action) => self.execute_vim(action, window, cx),
            _ => self.reroute(invocation, window, cx),
        }
    }

    pub(super) fn run(
        &mut self,
        invocation: &Invocation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let effect = self.workspace.update(cx, |workspace, cx| {
            cx.notify();
            workspace.run(invocation, &mut SystemClipboard(cx))
        });
        // El comando, o una macro antes de él, pudo cambiar los paneles.
        if effect != Effect::CloseWindow {
            self.sync_editors(cx);
        }
        self.apply(effect, window, cx);
    }

    fn apply(&mut self, effect: Effect, window: &mut Window, cx: &mut Context<Self>) {
        match effect {
            Effect::CloseWindow => {
                self.save_state(cx);
                window.remove_window();
            }
            Effect::Stay => {
                self.focus(window, cx);
                self.touch_focused(cx);
            }
            Effect::Window(command, argument) => {
                self.run_window_command(command, argument.as_deref(), window, cx);
            }
            Effect::FocusPastEdge(direction) => self.focus_past_edge(direction, window, cx),
            Effect::ChooseLink => self.open_link_choice(window, cx),
            Effect::Dispatch(action) => {
                window.dispatch_action(Box::new(Dispatch(action.into())), cx);
            }
            Effect::ChangeTags(request) => {
                self.vault_changes
                    .update(cx, |changes, cx| changes.change_tags(&request, cx));
            }
            Effect::OpenTagPage(tag) => self.open_tag_page(&tag, window, cx),
            Effect::SaveToggle { toggle, on } => {
                self.save_toggle(toggle, on, cx);
                self.focus(window, cx);
            }
        }
    }

    fn run_window_command(
        &mut self,
        command: WindowCommand,
        argument: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match command {
            WindowCommand::OpenFinder => self.open_finder(window, cx),
            WindowCommand::OpenTextSearch => self.open_text_search(window, cx),
            WindowCommand::OpenCommandPalette => self.open_palette(window, cx),
            WindowCommand::OpenUnresolvedLinks => self.open_unresolved_links(window, cx),
            WindowCommand::ToggleFolders => self.toggle_panel(PanelView::Folders, window, cx),
            WindowCommand::ToggleTags => self.toggle_panel(PanelView::Tags, window, cx),
            WindowCommand::ToggleContext => self.toggle_context(window, cx),
            WindowCommand::ToggleNoteList => self.toggle_list(window, cx),
            WindowCommand::ChooseListSource => self.choose_list_source(window, cx),
            WindowCommand::ShowFolder => self.show_folder(argument, window, cx),
            WindowCommand::SaveListView => {
                self.change_list_view(ListViewChange::Save, window, cx);
            }
            WindowCommand::RenameListView => {
                self.change_list_view(ListViewChange::Rename, window, cx);
            }
            WindowCommand::DeleteListView => {
                self.change_list_view(ListViewChange::Delete, window, cx);
            }
            WindowCommand::UndoVaultChange => {
                self.vault_changes.update(cx, VaultChanges::undo);
                self.focus(window, cx);
            }
            WindowCommand::ResumeTagChange => {
                // Una macro pudo cambiar los paneles antes.
                self.sync_editors(cx);
                self.vault_changes
                    .update(cx, VaultChanges::resume_tag_change);
            }
            WindowCommand::NewNoteFromTemplate => {
                self.new_note(NoteRequest::parse(argument), window, cx);
            }
            WindowCommand::OpenDailyNote => self.open_daily_note(window, cx),
            WindowCommand::NewBoard => self.new_board(argument, window, cx),
            WindowCommand::InsertNoteCard => self.open_card_picker(window, cx),
            WindowCommand::Focus => self.run_focus(argument, window, cx),
            WindowCommand::ClearFocus => self.set_focus(None, window, cx),
            WindowCommand::ShowMessages => self.open_messages(window, cx),
        }
    }

    /// `ctrl-w` hacia un lado sin panel vecino: lo que se vea fuera de los
    /// paneles de ese lado toma el foco.
    fn focus_past_edge(&self, direction: Direction, window: &mut Window, cx: &mut Context<Self>) {
        match direction {
            Direction::Left if self.list.read(cx).is_visible() => self.focus_list(window, cx),
            Direction::Left if self.sidebar.read(cx).is_visible() => {
                self.focus_sidebar(window, cx);
            }
            Direction::Right if self.context.read(cx).is_visible() => {
                self.focus_context(window, cx);
            }
            _ => {}
        }
    }
}
