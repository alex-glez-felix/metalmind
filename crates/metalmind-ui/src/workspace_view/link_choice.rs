//! `gd` sobre un link que empata entre varias notas: el picker de siempre
//! con las candidatas, en el orden en que empatan; la elegida se abre como
//! un salto en el panel enfocado.

use std::sync::Arc;

use gpui_kit::{AppContext as _, Context, Entity, Window};

use super::WorkspaceView;
use super::overlay::Overlay;
use crate::finder::Note;
use crate::picker::{Fuzzy, Haystack, PickerEvent, PickerOptions, PickerState, PickerView};

/// El picker de las notas entre las que empata un link.
pub type LinkChoicePicker = PickerView<Fuzzy<Note>>;

fn options() -> PickerOptions {
    PickerOptions {
        placeholder: "El link empata: elegir la nota…".into(),
        splits: false,
    }
}

impl WorkspaceView {
    pub(super) fn open_link_choice(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(choice) = self.workspace.read(cx).link_choice() else {
            return;
        };
        let notes: Arc<[Note]> = choice.candidates.iter().cloned().map(Note).collect();
        let fuzzy = Fuzzy::new(
            PickerState::new(notes),
            Haystack::Paths,
            "Ninguna nota".into(),
        );
        let picker = cx.new(|cx| PickerView::new(fuzzy, options(), cx));
        let events = cx.subscribe_in(&picker, window, Self::on_link_choice_event);
        self.show(
            Overlay::LinkChoice {
                picker,
                _events: events,
            },
            window,
            cx,
        );
    }

    fn on_link_choice_event(
        &mut self,
        _: &Entity<LinkChoicePicker>,
        event: &PickerEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let chosen = match event {
            PickerEvent::Dismissed => None,
            PickerEvent::Chosen(chosen) => Some(chosen.item),
        };
        self.close_overlay(window, cx);
        self.workspace.update(cx, |workspace, cx| {
            cx.notify();
            if let Err(error) = workspace.choose_link(chosen) {
                workspace.report(&error);
            }
        });
        self.focus(window, cx);
        self.touch_focused(cx);
    }
}
