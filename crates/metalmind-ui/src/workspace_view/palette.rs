//! La paleta en la ventana: lista las acciones disponibles desde el editor
//! enfocado y ejecuta la elegida como si fuera su atajo.

use std::sync::Arc;

use gpui_kit::{AppContext as _, Context, Entity, Window};
use metalmind_vim::{Action as VimAction, KeyOutcome};

use super::WorkspaceView;
use super::overlay::Overlay;
use crate::actions::{self, AppKeymap, Dispatch};
use crate::clipboard::SystemClipboard;
use crate::palette::{self, Entry, Target};
use crate::picker::{Fuzzy, Haystack, PickerEvent, PickerOptions, PickerState, PickerView};

/// La paleta de comandos.
pub type Palette = PickerView<Fuzzy<Entry>>;

impl WorkspaceView {
    /// Se llama con el foco en un editor: sus acciones son las que se listan.
    pub(super) fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let entries: Arc<[Entry]> = palette::entries(window, AppKeymap::get(cx), cx).into();
        let options = PickerOptions {
            placeholder: "Ejecutar una acción…".into(),
            splits: false,
        };
        let fuzzy = Fuzzy::new(
            PickerState::new(entries),
            Haystack::Text,
            "No hay acciones".into(),
        );
        let picker = cx.new(|cx| PickerView::new(fuzzy, options, cx));
        let events = cx.subscribe_in(&picker, window, Self::on_palette_event);
        self.show(
            Overlay::Palette {
                picker,
                _events: events,
            },
            window,
            cx,
        );
    }

    fn on_palette_event(
        &mut self,
        picker: &Entity<Palette>,
        event: &PickerEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = match event {
            PickerEvent::Dismissed => None,
            PickerEvent::Chosen(chosen) => picker
                .read(cx)
                .delegate()
                .items()
                .get(chosen.item)
                .map(|entry| entry.target),
        };
        self.close_overlay(window, cx);
        match target {
            Some(Target::Registry(action)) => {
                window.dispatch_action(Box::new(Dispatch(action.into())), cx);
            }
            Some(Target::Window(name)) => match cx.build_action(name, None) {
                Ok(action) => window.dispatch_action(action, cx),
                Err(error) => log::error!("la paleta no pudo construir {name}: {error:#}"),
            },
            None => {}
        }
    }

    /// Como si se tecleara el atajo de `action` en el panel enfocado.
    pub(super) fn execute_vim(
        &mut self,
        action: VimAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let outcome = self.workspace.update(cx, |workspace, cx| {
            cx.notify();
            let pane = workspace.focused();
            workspace.execute(pane, action, &mut SystemClipboard(cx))
        });
        match outcome {
            Ok(KeyOutcome::Dispatch(invocation)) => {
                let name = invocation.action.as_str();
                if let Err(error) = actions::dispatch(name, invocation.argument, window, cx) {
                    self.workspace
                        .update(cx, |workspace, _| workspace.report(&error.into()));
                }
            }
            Ok(_) => {}
            Err(error) => self
                .workspace
                .update(cx, |workspace, _| workspace.report(&error)),
        }
    }
}
