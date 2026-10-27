//! `space u` en la ventana: el picker de los links sin destino del vault;
//! el elegido abre su nota con el cursor en el link, en el panel o en un
//! split, como `space /`.

use gpui_kit::{AppContext as _, Context, Entity, Window};

use super::WorkspaceView;
use super::overlay::Overlay;
use crate::picker::{Fuzzy, Haystack, PickerEvent, PickerOptions, PickerState, PickerView};
use crate::unresolved::{UnresolvedLink, unresolved_links};

/// El picker de `space u`.
pub type UnresolvedPicker = PickerView<Fuzzy<UnresolvedLink>>;

fn options() -> PickerOptions {
    PickerOptions {
        placeholder: "Links sin destino…".into(),
        splits: true,
    }
}

impl WorkspaceView {
    pub(super) fn open_unresolved_links(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let links = unresolved_links(&self.vault.session.read(cx).vault().links());
        let fuzzy = Fuzzy::new(
            PickerState::new(links),
            Haystack::Text,
            "Todos los links llevan a algo".into(),
        );
        let picker = cx.new(|cx| PickerView::new(fuzzy, options(), cx));
        let events = cx.subscribe_in(&picker, window, Self::on_unresolved_event);
        self.show(
            Overlay::Unresolved {
                picker,
                _events: events,
            },
            window,
            cx,
        );
    }

    fn on_unresolved_event(
        &mut self,
        picker: &Entity<UnresolvedPicker>,
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
                .map(|link| (link.clone(), chosen.open)),
        };
        self.close_overlay(window, cx);
        if let Some((link, open)) = chosen {
            self.open_note_at(&link.source, link.at, open, window, cx);
        }
    }

    /// Con el picker abierto, un lote del vault (una nota nueva que resuelve
    /// un link, otra con links nuevos) cambia la lista.
    pub(super) fn refresh_unresolved_links(&mut self, cx: &mut Context<Self>) {
        let Some(Overlay::Unresolved { picker, .. }) = &self.overlay else {
            return;
        };
        let links = unresolved_links(&self.vault.session.read(cx).vault().links());
        picker.update(cx, |picker, cx| {
            picker.update_delegate(cx, |fuzzy, cx| fuzzy.set_items(links, cx));
        });
    }
}
