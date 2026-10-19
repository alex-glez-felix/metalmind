//! `space /` en la ventana: abre el picker de texto sobre el vault y abre
//! la nota elegida en la línea del resultado, en el panel o en un split,
//! como `space f`.

use gpui_kit::{AppContext as _, Context, Entity, Window};

use super::WorkspaceView;
use super::overlay::Overlay;
use crate::picker::{Open, PickerEvent, PickerOptions, PickerView};
use crate::text_search::{Hit, OpenNote, TextSearch};
use crate::workspace::{Axis, LinePosition, Placement, shown_path};

/// El picker de `space /`.
pub type TextSearchPicker = PickerView<TextSearch>;

impl WorkspaceView {
    pub(super) fn open_text_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let session = self.vault.session.read(cx);
        let root = session.root().to_owned();
        let open: Vec<OpenNote> = self
            .workspace
            .read(cx)
            .unsaved_files()
            .filter_map(|(path, buffer)| {
                Some(OpenNote {
                    path: session.vault_path(path)?,
                    text: buffer.to_string(),
                })
            })
            .collect();
        let options = PickerOptions {
            placeholder: "Buscar en las notas…".into(),
            splits: true,
        };
        let picker = cx.new(|cx| {
            let search = TextSearch::new(&root, open, cx);
            PickerView::new(search, options, cx)
        });
        let events = cx.subscribe_in(&picker, window, Self::on_text_search_event);
        self.show(
            Overlay::TextSearch {
                picker,
                _events: events,
            },
            window,
            cx,
        );
    }

    fn on_text_search_event(
        &mut self,
        picker: &Entity<TextSearchPicker>,
        event: &PickerEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let chosen = match event {
            PickerEvent::Dismissed => None,
            PickerEvent::Chosen(chosen) => picker
                .read(cx)
                .delegate()
                .hit(chosen.item)
                .map(|hit| (hit.clone(), chosen.open)),
        };
        self.close_overlay(window, cx);
        if let Some((hit, open)) = chosen {
            self.open_hit(&hit, open, window, cx);
        }
    }

    /// Como `:e ruta`, o `:vsplit ruta` y `:split ruta`, con el cursor en
    /// la primera coincidencia de la línea.
    fn open_hit(&mut self, hit: &Hit, open: Open, window: &mut Window, cx: &mut Context<Self>) {
        let root = self.vault.session.read(cx).root().to_owned();
        let path = shown_path(&root.join(hit.path.as_str()));
        let placement = match open {
            Open::Here => Placement::Current,
            Open::SplitRight => Placement::Split(Axis::Row),
            Open::SplitDown => Placement::Split(Axis::Column),
        };
        let at = LinePosition {
            line: hit.line.line,
            column: hit.column(),
        };
        let opened = self.workspace.update(cx, |workspace, cx| {
            cx.notify();
            workspace.open_file_at(&path, placement, at)
        });
        if let Err(error) = opened {
            self.workspace
                .update(cx, |workspace, _| workspace.report(&error));
        }
        self.sync_editors(cx);
        self.focus(window, cx);
        self.touch_focused(cx);
    }
}
