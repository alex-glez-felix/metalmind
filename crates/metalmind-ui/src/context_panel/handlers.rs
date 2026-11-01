//! Qué hace cada acción `context::` sobre el panel.

use gpui_kit::{Context, Window};
use metalmind_keymap::{Action, ContextAction};

use super::{ContextEvent, ContextPanel};
use crate::actions::Dispatch;
use crate::workspace::{Axis, Placement};

impl ContextPanel {
    /// Las acciones del panel; las demás siguen hacia la raíz de la ventana.
    pub(super) fn on_dispatch(
        &mut self,
        Dispatch(invocation): &Dispatch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Action::Context(action) = invocation.action else {
            cx.propagate();
            return;
        };
        window.focus(&self.focus_handle, cx);
        let outline = self.context.outline_mut();
        match action {
            ContextAction::SelectNext => outline.select_next(),
            ContextAction::SelectPrevious => outline.select_previous(),
            ContextAction::SelectFirst => outline.select_first(),
            ContextAction::SelectLast => outline.select_last(),
            ContextAction::CollapseOrParent => outline.collapse_or_parent(),
            ContextAction::ExpandOrOpen => {
                if let Some(open) = self.context.expand_or_open() {
                    self.open(&open, Placement::Current, cx);
                }
            }
            ContextAction::Open => self.open_selected(Placement::Current, cx),
            ContextAction::OpenSplitRight => self.open_selected(Placement::Split(Axis::Row), cx),
            ContextAction::OpenSplitDown => self.open_selected(Placement::Split(Axis::Column), cx),
            ContextAction::NextTab => self.context.set_tab(self.context.tab().next()),
            ContextAction::Close => return self.close(cx),
            ContextAction::FocusEditor => cx.emit(ContextEvent::FocusEditor),
        }
        self.scroll_to_selection();
        cx.notify();
    }

    fn open_selected(&mut self, placement: Placement, cx: &mut Context<Self>) {
        if let Some(open) = self.context.selected_open(true) {
            self.open(&open, placement, cx);
        }
    }
}
