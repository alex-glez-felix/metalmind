//! Qué hace cada acción `list::` sobre la lista.

use gpui_kit::{Context, Window};
use metalmind_keymap::{Action, ListAction};

use super::model::Prompt;
use super::view::{NoteListEvent, NoteListView};
use crate::actions::Dispatch;
use crate::picker::LineEdit;
use crate::workspace::{Axis, Placement};

impl NoteListView {
    /// Las acciones de la lista; las demás siguen hacia la raíz de la ventana.
    pub(super) fn on_dispatch(
        &mut self,
        Dispatch(invocation): &Dispatch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Action::List(action) = invocation.action else {
            cx.propagate();
            return;
        };
        window.focus(&self.focus_handle, cx);
        match action {
            ListAction::SelectNext => self.list.select_next(1),
            ListAction::SelectPrevious => self.list.select_previous(1),
            ListAction::SelectFirst => self.list.select_first(),
            ListAction::SelectLast => self.list.select_last(),
            ListAction::Open => self.open_selected(Placement::Current, true, cx),
            ListAction::OpenSplitRight => {
                self.open_selected(Placement::Split(Axis::Row), true, cx);
            }
            ListAction::OpenSplitDown => {
                self.open_selected(Placement::Split(Axis::Column), true, cx);
            }
            ListAction::Filter => self.list.start_filter(),
            ListAction::Tag => {
                self.list.start_tagging();
            }
            ListAction::Confirm => self.confirm(cx),
            ListAction::Cancel => self.cancel(cx),
            ListAction::FocusEditor => cx.emit(NoteListEvent::FocusEditor),
            ListAction::FocusSidebar => cx.emit(NoteListEvent::FocusSidebar),
        }
        self.scroll_to_selection();
        cx.notify();
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        // Un preedit sin confirmar no pasa a la pregunta siguiente.
        self.marked = None;
        match self.list.prompt() {
            Some(Prompt::Filter) | None => self.list.close_prompt(),
            Some(Prompt::Tag) => {
                if let Some((note, tags)) = self.list.take_tags() {
                    cx.emit(NoteListEvent::TagNote { note, tags });
                }
            }
        }
    }

    /// Cierra la pregunta; sin pregunta, quita el filtro.
    fn cancel(&mut self, cx: &mut Context<Self>) {
        if self.list.prompt().is_some() {
            self.list.close_prompt();
            self.marked = None;
            return;
        }
        self.scroll_on_show = true;
        // Al final, borrar hasta el principio es borrarlo todo.
        let _ = self.list.edit_filter(LineEdit::End);
        let step = self.list.edit_filter(LineEdit::DeleteToStart);
        self.step(step, cx);
    }
}
