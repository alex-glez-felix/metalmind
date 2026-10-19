//! Cuándo corre el autoguardado del [`Workspace`]: un temporizador hasta su
//! próximo plazo y cuando la ventana pierde el foco. Lo de cerrar la
//! ventana lo hace el propio workspace antes de decidir si se puede.

use std::time::Instant;

use gpui_kit::{Context, Task, Window};

use super::WorkspaceView;
#[cfg(doc)]
use crate::workspace::Workspace;

/// El temporizador que espera hasta `at`; soltarlo lo cancela.
pub(super) struct AutosaveTimer {
    at: Instant,
    _task: Task<()>,
}

impl WorkspaceView {
    /// Deja un temporizador para el plazo de [`Workspace::autosave_deadline`],
    /// si cambió: cada edición lo corre.
    pub(super) fn schedule_autosave(&mut self, cx: &mut Context<Self>) {
        let deadline = self.workspace.read(cx).autosave_deadline();
        if self.autosave.as_ref().map(|timer| timer.at) == deadline {
            return;
        }
        self.autosave = deadline.map(|at| {
            let task = cx.spawn(async move |view, cx| {
                let wait = at.saturating_duration_since(Instant::now());
                cx.background_executor().timer(wait).await;
                // Si la vista ya no existe, no hay nada que guardar desde aquí.
                let _ = view.update(cx, Self::autosave_due);
            });
            AutosaveTimer { at, _task: task }
        });
    }

    fn autosave_due(&mut self, cx: &mut Context<Self>) {
        self.autosave = None;
        self.workspace.update(cx, |workspace, cx| {
            if workspace.autosave_if_due() {
                cx.notify();
            }
        });
        self.schedule_autosave(cx);
    }

    pub(super) fn on_activation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if window.is_window_active() {
            return;
        }
        self.workspace.update(cx, |workspace, cx| {
            if workspace.autosave_all() {
                cx.notify();
            }
        });
    }
}
