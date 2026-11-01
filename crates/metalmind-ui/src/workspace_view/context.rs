//! El panel de contexto en la ventana: mostrarlo o cerrarlo y a dónde va el
//! foco cuando abre una nota o se cierra.

use gpui_kit::{Context, Window};

use super::WorkspaceView;
use crate::context_panel::ContextEvent;

impl WorkspaceView {
    pub(super) fn toggle_context(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let visible = self
            .context
            .update(cx, |context, cx| context.toggle(window, cx));
        if visible {
            self.focus_context(window, cx);
        } else {
            self.focus(window, cx);
        }
        cx.notify();
    }

    pub(super) fn focus_context(&self, window: &mut Window, cx: &mut Context<Self>) {
        let handle = self.context.read(cx).focus_handle().clone();
        window.focus(&handle, cx);
    }

    pub(super) fn on_context_event(
        &mut self,
        event: &ContextEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match *event {
            ContextEvent::Opened { focus_editor } => {
                self.sync_editors(cx);
                if focus_editor {
                    self.focus(window, cx);
                }
                self.touch_focused(cx);
            }
            ContextEvent::FocusEditor => self.focus(window, cx),
        }
        cx.notify();
    }
}
