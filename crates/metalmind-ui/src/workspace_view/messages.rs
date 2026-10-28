//! `:messages`: el historial de avisos en el picker de siempre, del más
//! nuevo al más viejo, con su nivel; el elegido vuelve a verse en un toast.

use std::sync::Arc;

use gpui_kit::{AppContext as _, Context, Entity, Window};

use super::WorkspaceView;
use super::overlay::Overlay;
use crate::picker::{
    Fuzzy, Haystack, PickerEvent, PickerItem, PickerOptions, PickerState, PickerView,
};
use crate::workspace::{Message, MessageId};

/// El picker del historial de avisos.
pub type MessagesPicker = PickerView<Fuzzy<Logged>>;

/// Un aviso del historial.
pub struct Logged {
    id: MessageId,
    text: String,
    level: &'static str,
}

impl Logged {
    fn new(id: MessageId, message: &Message) -> Self {
        let level = match message {
            Message::Info(_) => "info",
            Message::Warning(_) => "aviso",
            Message::Error(_) => "error",
        };
        Self {
            id,
            text: message.text().to_owned(),
            level,
        }
    }
}

impl AsRef<str> for Logged {
    fn as_ref(&self) -> &str {
        &self.text
    }
}

impl PickerItem for Logged {
    fn detail(&self) -> Option<&str> {
        Some(self.level)
    }
}

impl WorkspaceView {
    pub(super) fn open_messages(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let logged: Arc<[Logged]> = self
            .workspace
            .read(cx)
            .messages()
            .history()
            .map(|(id, message)| Logged::new(id, message))
            .collect();
        let fuzzy = Fuzzy::new(
            PickerState::new(logged),
            Haystack::Text,
            "No hay avisos".into(),
        );
        let options = PickerOptions {
            placeholder: "Buscar en los avisos…".into(),
            splits: false,
            focus: None,
        };
        let picker = cx.new(|cx| PickerView::new(fuzzy, options, cx));
        let events = cx.subscribe_in(&picker, window, Self::on_messages_event);
        self.show(
            Overlay::Messages {
                picker,
                _events: events,
            },
            window,
            cx,
        );
    }

    fn on_messages_event(
        &mut self,
        picker: &Entity<MessagesPicker>,
        event: &PickerEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let chosen = match event {
            PickerEvent::Dismissed => None,
            PickerEvent::Chosen(chosen) => picker
                .read(cx)
                .delegate()
                .items()
                .get(chosen.item)
                .map(|logged| logged.id),
        };
        self.close_overlay(window, cx);
        if let Some(id) = chosen {
            self.workspace.update(cx, |workspace, cx| {
                workspace.reshow_message(id);
                cx.notify();
            });
        }
    }
}
