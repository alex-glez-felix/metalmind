//! Los comandos en la ventana: una acción que llega a la raíz se ejecuta en
//! el [`Workspace`](crate::workspace::Workspace), y lo que este pide hacer
//! fuera de él ([`Effect`]) lo hace cada submódulo de la ventana.

use gpui_kit::{Context, Window};
use metalmind_keymap::{Action, Invocation};

use super::WorkspaceView;
use crate::actions::Dispatch;
use crate::clipboard::SystemClipboard;
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
            Action::Command(_) => self.run(invocation, window, cx),
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
            Effect::OpenFinder => self.open_finder(window, cx),
            Effect::OpenTextSearch => self.open_text_search(window, cx),
            Effect::OpenCommandPalette => self.open_palette(window, cx),
            Effect::OpenUnresolvedLinks => self.open_unresolved_links(window, cx),
            Effect::TogglePanel(view) => self.toggle_panel(view, window, cx),
            Effect::ToggleContext => self.toggle_context(window, cx),
            Effect::ToggleNoteList => self.toggle_list(window, cx),
            Effect::ChooseListSource => self.choose_list_source(window, cx),
            Effect::ShowFolder(folder) => self.show_folder(folder.as_deref(), window, cx),
            Effect::ListView(change) => self.change_list_view(change, window, cx),
            Effect::FocusPastEdge(direction) => self.focus_past_edge(direction, window, cx),
            Effect::ChooseLink => self.open_link_choice(window, cx),
            Effect::Dispatch(action) => {
                window.dispatch_action(Box::new(Dispatch(action.into())), cx);
            }
            Effect::ChangeTags(request) => self.change_tags(&request, window, cx),
            Effect::UndoVaultChange => {
                self.undo_vault_change(cx);
                self.focus(window, cx);
            }
            Effect::ResumeTagChange => self.resume_tag_change(window, cx),
            Effect::NewNote(request) => self.new_note(request, window, cx),
            Effect::OpenDailyNote => self.open_daily_note(window, cx),
            Effect::OpenTagPage(tag) => self.open_tag_page(&tag, window, cx),
            Effect::Focus(command) => self.run_focus(command, window, cx),
            Effect::OpenMessages => self.open_messages(window, cx),
            Effect::SaveToggle { toggle, on } => {
                self.save_toggle(toggle, on, cx);
                self.focus(window, cx);
            }
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
