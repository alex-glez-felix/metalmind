//! Qué tooltip de marks muestra el panel: el de la línea bajo el ratón, tras
//! la misma demora que los tooltips de GPUI.

use std::time::Duration;

use gpui_kit::{Context, Task};

use super::Editor;

/// La de `div().tooltip()` en GPUI, para que todos los tooltips tarden igual.
const DELAY: Duration = Duration::from_millis(500);

pub(super) struct MarkHover {
    pub line: usize,
    pub shown: bool,
    _delay: Task<()>,
}

impl Editor {
    /// El ratón entró a la zona de las marks de `line` o salió de ella.
    pub(super) fn hover_mark(&mut self, line: usize, hovered: bool, cx: &mut Context<Self>) {
        if !hovered {
            if self
                .mark_hover
                .as_ref()
                .is_some_and(|hover| hover.line == line)
            {
                self.mark_hover = None;
                cx.notify();
            }
            return;
        }
        let delay = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(DELAY).await;
            // Si el panel ya no existe, no hay nada que mostrar.
            let _ = this.update(cx, |editor, cx| {
                if let Some(hover) = &mut editor.mark_hover {
                    hover.shown = true;
                    cx.notify();
                }
            });
        });
        self.mark_hover = Some(MarkHover {
            line,
            shown: false,
            _delay: delay,
        });
    }

    /// La línea cuyo tooltip de marks se ve.
    pub(super) fn shown_mark_tooltip(&self) -> Option<usize> {
        self.mark_hover
            .as_ref()
            .filter(|hover| hover.shown)
            .map(|hover| hover.line)
    }
}
