//! La lista de notas en la ventana: plegarla, elegir su fuente (también una
//! vista guardada) en un picker
//! y etiquetar la nota elegida, que se aplica y se deshace (`:vaultundo`) como
//! un cambio de tags (05 §5).

use std::sync::Arc;

use gpui_kit::{AppContext as _, Context, Entity, Window};
use metalmind_vault::{Changes, VaultPath};

use super::WorkspaceView;
use super::overlay::Overlay;
use crate::note_list::{ListSource, NoteListEvent, NoteListView, tag_edit};
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
        self.show(
            Overlay::ListSource {
                picker,
                _events: events,
            },
            window,
            cx,
        );
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
            NoteListEvent::TagNote { note, tags } => self.tag_note(&note, &tags, cx),
        }
        cx.notify();
    }

    /// Agrega `tags` al frontmatter de `note`. Como un cambio de tags, no
    /// escribe por debajo de un buffer con cambios y `:vaultundo` lo deshace.
    fn tag_note(&mut self, note: &VaultPath, tags: &[String], cx: &mut Context<Self>) {
        if self.unsaved_paths(cx).contains(note) {
            let text = format!("no se etiquetó {note}: tiene cambios sin guardar; guárdala antes");
            return self.tell(Message::Error(text), cx);
        }
        let root = self.session.read(cx).root().to_owned();
        let text = match metalmind_vault::read(&root.join(note.as_str())) {
            Ok(Some(text)) => text,
            Ok(None) => String::new(),
            Err(error) => return self.tell(Message::Error(error.to_string()), cx),
        };
        let edits = match tag_edit(note, &text, tags) {
            Ok(Some(edits)) => edits,
            Ok(None) => {
                let text = format!("{note} ya tiene {}", tags.join(" "));
                return self.tell(Message::Info(text), cx);
            }
            Err(error) => {
                let text = format!("no se etiquetó {note}: {error}");
                return self.tell(Message::Error(text), cx);
            }
        };
        let changes = Changes {
            edits: vec![edits],
            ..Changes::default()
        };
        let summary = format!("{note}: {}", tags.join(" "));
        self.apply_tag_changes(format!("etiquetar {note}"), &summary, &changes, cx);
    }
}
