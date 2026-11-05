//! La lista de notas en la ventana: plegarla y elegir su fuente (también una
//! vista guardada) en un picker.

use std::sync::Arc;

use gpui_kit::{AppContext as _, Context, Entity, Window};

use super::WorkspaceView;
use super::overlay::Overlay;
use crate::note_list::{ListSource, NoteListEvent, NoteListView};
use crate::picker::{
    Fuzzy, Haystack, PickerEvent, PickerItem, PickerOptions, PickerState, PickerView,
};
use crate::workspace::Message;

/// Una fuente en el picker de `space v`, por su nombre.
pub struct SourceChoice {
    source: ListSource,
    label: String,
    /// La consulta de una vista guardada.
    detail: Option<String>,
}

impl AsRef<str> for SourceChoice {
    fn as_ref(&self) -> &str {
        &self.label
    }
}

impl PickerItem for SourceChoice {
    fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }
}

pub type SourcePicker = PickerView<Fuzzy<SourceChoice>>;

impl WorkspaceView {
    pub(super) fn toggle_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let visible = self.list.update(cx, NoteListView::toggle);
        if visible {
            self.focus_list(window, cx);
        } else {
            self.focus(window, cx);
        }
        cx.notify();
    }

    pub(super) fn focus_list(&self, window: &mut Window, cx: &mut Context<Self>) {
        let handle = self.list.read(cx).focus_handle().clone();
        window.focus(&handle, cx);
    }

    pub(super) fn choose_list_source(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let views = self.vault_views(cx).unwrap_or_default();
        let vault = self.session.read(cx).vault();
        let tags = self.session.read(cx).tags();
        let choices: Arc<[SourceChoice]> = ListSource::choices(&vault.snapshot(), &tags, &views)
            .into_iter()
            .map(|source| SourceChoice {
                label: source.label(),
                detail: source.view_name().and(source.query()),
                source,
            })
            .collect();
        let options = PickerOptions {
            placeholder: "Mostrar en la lista…".into(),
            splits: false,
            focus: None,
        };
        let fuzzy = Fuzzy::new(
            PickerState::new(choices),
            Haystack::Paths,
            "No hay fuentes".into(),
        );
        let picker = cx.new(|cx| PickerView::new(fuzzy, options, cx));
        // Al cancelar, el foco vuelve a la lista si se abrió desde ella.
        let from_list = self
            .list
            .read(cx)
            .focus_handle()
            .contains_focused(window, cx);
        let events = cx.subscribe_in(&picker, window, move |this, picker, event, window, cx| {
            this.on_source_event(picker, event, from_list, window, cx);
        });
        self.show(Overlay::new(picker, vec![events], cx), window, cx);
    }

    /// `list::show_folder`: la lista muestra la carpeta y toma el foco.
    pub(super) fn show_folder(
        &mut self,
        named: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.list.update(cx, |list, cx| list.show_folder(named, cx)) {
            Ok(()) => self.focus_list(window, cx),
            Err(error) => self.tell(Message::Error(error.to_string()), cx),
        }
        cx.notify();
    }

    fn on_source_event(
        &mut self,
        picker: &Entity<SourcePicker>,
        event: &PickerEvent,
        from_list: bool,
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
                .map(|choice| choice.source.clone()),
        };
        self.overlay = None;
        match chosen {
            Some(source) => {
                self.list.update(cx, |list, cx| list.set_source(source, cx));
                self.focus_list(window, cx);
            }
            None if from_list => self.focus_list(window, cx),
            None => self.focus(window, cx),
        }
        cx.notify();
    }

    pub(super) fn on_list_event(
        &mut self,
        event: NoteListEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            NoteListEvent::Opened { focus_editor } => {
                self.sync_editors(cx);
                if focus_editor {
                    self.focus(window, cx);
                }
                self.touch_focused(cx);
            }
            NoteListEvent::FocusEditor => self.focus(window, cx),
            NoteListEvent::FocusSidebar if self.sidebar_visible(cx) => {
                self.focus_sidebar(window, cx);
            }
            NoteListEvent::FocusSidebar => {}
            NoteListEvent::TagNote { note, tags } => {
                self.vault_changes
                    .update(cx, |changes, cx| changes.tag_note(&note, &tags, cx));
            }
        }
        cx.notify();
    }
}
