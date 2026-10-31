//! Las vistas guardadas desde la lista (06 §10): "Guardar como vista" con la
//! fuente y el filtro, renombrar y borrar la que se muestra. Se escriben en
//! la config del vault y la lista las sigue.

use gpui_kit::{AppContext as _, Context, Entity, Window};
use metalmind_config::{SavedView, VaultConfig};

use super::WorkspaceView;
use super::overlay::Overlay;
use crate::note_list::{
    ListSource, ViewDraft, ViewNamePurpose, ViewNameQuestion, ViewsError, change_views, load_views,
    view_named,
};
use crate::picker::{PickerEvent, PickerOptions, PickerView};
use crate::workspace::{ListViewChange, Message};

pub type ViewNamePicker = PickerView<ViewNameQuestion>;

impl WorkspaceView {
    pub(super) fn change_list_view(
        &mut self,
        change: ListViewChange,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let source = self.list.read(cx).state().source;
        let purpose = match (change, &source) {
            (ListViewChange::Save, _) => {
                let filter = self.list.read(cx).state().filter;
                match ViewDraft::new(&source, &filter) {
                    Ok(draft) => {
                        if !draft.dropped.is_empty() {
                            let text = format!(
                                "la vista guarda solo tags; no lleva {}",
                                draft.dropped.join(" ")
                            );
                            self.tell(Message::Info(text), cx);
                        }
                        ViewNamePurpose::Save { query: draft.query }
                    }
                    Err(error) => return self.tell(Message::Error(error.to_string()), cx),
                }
            }
            (ListViewChange::Rename, ListSource::View { name, query }) => ViewNamePurpose::Rename {
                from: name.clone(),
                query: query.clone(),
            },
            (ListViewChange::Delete, ListSource::View { name, query }) => {
                return self.delete_view(name, query, cx);
            }
            (ListViewChange::Rename | ListViewChange::Delete, _) => {
                let text = "la lista no muestra una vista guardada; elige una con space v";
                return self.tell(Message::Error(text.to_owned()), cx);
            }
        };
        let Some(views) = self.vault_views(cx) else {
            return;
        };
        self.ask_view_name(ViewNameQuestion::new(purpose, &views), window, cx);
    }

    /// Las vistas del vault; si su config está rota, lo dice y da `None`.
    pub(super) fn vault_views(&mut self, cx: &mut Context<Self>) -> Option<Vec<SavedView>> {
        let root = self.session.read(cx).root().to_owned();
        let (views, diagnostics) = load_views(&root);
        match diagnostics
            .into_iter()
            .find(|diagnostic| diagnostic.severity == metalmind_config::Severity::Error)
        {
            Some(error) => {
                self.tell(Message::Error(error.to_string()), cx);
                None
            }
            None => Some(views),
        }
    }

    fn ask_view_name(
        &mut self,
        question: ViewNameQuestion,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let placeholder = match question.purpose() {
            ViewNamePurpose::Save { .. } => "Nombre de la vista nueva…",
            ViewNamePurpose::Rename { .. } => "Nombre nuevo de la vista…",
        };
        let options = PickerOptions {
            placeholder: placeholder.into(),
            splits: false,
            focus: None,
        };
        let picker = cx.new(|cx| PickerView::new(question, options, cx));
        let events = cx.subscribe_in(&picker, window, Self::on_view_name_event);
        self.show(
            Overlay::ViewName {
                picker,
                _events: events,
            },
            window,
            cx,
        );
    }

    fn on_view_name_event(
        &mut self,
        picker: &Entity<ViewNamePicker>,
        event: &PickerEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let answer = match event {
            PickerEvent::Dismissed => None,
            PickerEvent::Chosen(_) => {
                let question = picker.read(cx).delegate();
                question
                    .answer()
                    .ok()
                    .map(|name| (name.to_owned(), question.purpose().clone()))
            }
        };
        self.overlay = None;
        self.focus_list(window, cx);
        cx.notify();
        let Some((name, purpose)) = answer else {
            return;
        };
        match purpose {
            ViewNamePurpose::Save { query } => self.save_view(&name, &query, cx),
            ViewNamePurpose::Rename { from, .. } => self.rename_view(&from, &name, cx),
        }
    }

    fn save_view(&mut self, name: &str, query: &str, cx: &mut Context<Self>) {
        let root = self.session.read(cx).root().to_owned();
        let saved = change_views(&root, |text, current| {
            if let Some(taken) = view_named(current.views(), name) {
                return Err(ViewsError::Exists(taken.name.clone()));
            }
            Ok(VaultConfig::save_view(text, name, query)?)
        });
        let view = match saved {
            Ok(views) => views.into_iter().find(|view| view.name == name),
            Err(error) => return self.tell(Message::Error(error.to_string()), cx),
        };
        if let Some(view) = view {
            self.list
                .update(cx, |list, cx| list.show_saved_view(&view, cx));
            let text = format!("se guardó la vista «{name}»: {query}");
            self.tell(Message::Info(text), cx);
        }
    }

    fn rename_view(&mut self, from: &str, to: &str, cx: &mut Context<Self>) {
        let root = self.session.read(cx).root().to_owned();
        let renamed = change_views(&root, |text, current| {
            if let Some(taken) = view_named(current.views(), to).filter(|view| view.name != from) {
                return Err(ViewsError::Exists(taken.name.clone()));
            }
            if current.view(from).is_none() {
                return Err(ViewsError::Missing(from.to_owned()));
            }
            Ok(VaultConfig::rename_view(text, from, to)?)
        });
        match renamed {
            Ok(views) => {
                self.list.update(cx, |list, cx| {
                    list.follow_views(&views, Some((from, to)), cx);
                });
                let text = format!("la vista «{from}» se llama ahora «{to}»");
                self.tell(Message::Info(text), cx);
            }
            Err(error) => self.tell(Message::Error(error.to_string()), cx),
        }
    }

    fn delete_view(&mut self, name: &str, query: &str, cx: &mut Context<Self>) {
        let root = self.session.read(cx).root().to_owned();
        let deleted = change_views(&root, |text, current| {
            if current.view(name).is_none() {
                return Err(ViewsError::Missing(name.to_owned()));
            }
            Ok(VaultConfig::delete_view(text, name)?)
        });
        match deleted {
            Ok(views) => {
                self.list
                    .update(cx, |list, cx| list.follow_views(&views, None, cx));
                let text = format!("se borró la vista «{name}» ({query}); sus notas no cambian");
                self.tell(Message::Info(text), cx);
            }
            Err(error) => self.tell(Message::Error(error.to_string()), cx),
        }
    }
}
