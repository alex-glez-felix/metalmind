//! `:template`, `space n`, una nota nueva desde un tag y `:today` en la
//! ventana: lee las plantillas del vault, muestra el picker de plantillas y
//! la pregunta del nombre, crea la nota y la abre con el cursor donde
//! empieza a escribirse.

use std::sync::Arc;

use gpui_kit::{AppContext as _, Context, Entity, Window};
use jiff::Zoned;
use metalmind_template::Templates;

use super::WorkspaceView;
use super::overlay::Overlay;
use metalmind_template::{Draft, NotePath};
use metalmind_vault::{FileOpError, VaultPath};

use crate::focus::FocusedNote;
use crate::new_note::{
    self, Landing, NameProblem, NameQuestion, NoteRequest, Origin, TemplateItem, template_items,
};
use crate::picker::{
    Fuzzy, Haystack, LineEdit, Open, PickerEvent, PickerOptions, PickerState, PickerView,
};
use crate::workspace::{Axis, Message, Placement, shown_path};

pub type TemplatePicker = PickerView<Fuzzy<TemplateItem>>;
pub type NamePicker = PickerView<NameQuestion>;

impl WorkspaceView {
    pub(super) fn new_note(
        &mut self,
        request: NoteRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(templates) = self.templates(cx) else {
            return;
        };
        let Some(template) = request.template else {
            return self.choose_template(templates, window, cx);
        };
        if templates.get(&template).is_none() {
            let folder = &self.notes.templates_folder;
            let text = format!("no existe la plantilla `{template}` en {folder}/");
            return self.tell(Message::Error(text), cx);
        }
        let mut question = self.question(Origin::Template(template), templates, cx);
        let Some(name) = request.name else {
            return self.ask_name(question, window, cx);
        };
        question.apply(LineEdit::Insert(&name));
        if let Err(problem) = self.create(&question, Open::Here, window, cx) {
            self.tell(Message::Error(problem.to_string()), cx);
        }
    }

    /// `a` sobre un tag del panel lateral: una nota con ese tag.
    pub(super) fn new_tagged_note(
        &mut self,
        tag: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(templates) = self.templates(cx) {
            let question = self.question(Origin::Tag(tag), templates, cx);
            self.ask_name(question, window, cx);
        }
    }

    pub(super) fn open_daily_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let vault = self.session.read(cx).vault();
        match new_note::open_daily(&vault, &self.notes, &Zoned::now()) {
            Ok(landing) => self.land(&landing, Placement::Current, window, cx),
            Err(error) => self.tell(Message::Error(error.to_string()), cx),
        }
    }

    /// Las plantillas del vault; si la carpeta de la config no vale, lo
    /// dice y da `None`.
    fn templates(&mut self, cx: &mut Context<Self>) -> Option<Templates> {
        let session = self.session.read(cx);
        let vault = session.vault();
        let folder = &self.notes.templates_folder;
        match new_note::load_templates(&vault, &vault.snapshot(), folder) {
            Ok(templates) => Some(templates),
            Err(error) => {
                self.tell(Message::Error(error.to_string()), cx);
                None
            }
        }
    }

    fn question(&self, origin: Origin, templates: Templates, cx: &Context<Self>) -> NameQuestion {
        let index = self.session.read(cx).vault().snapshot();
        NameQuestion::new(origin, templates, index).in_focus(self.focused_note(cx))
    }

    /// Dónde y con qué tags nace una nota nueva en el focus de ahora.
    fn focused_note(&self, cx: &Context<Self>) -> FocusedNote {
        let session = self.session.read(cx);
        session
            .focus()
            .map(|focus| focus.new_note(&session.tags()))
            .unwrap_or_default()
    }

    /// `a` en el panel lateral: lo que salga mal se dice en el panel, donde
    /// se pidió.
    pub(super) fn create_panel_note(
        &mut self,
        path: &VaultPath,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let session = self.session.read(cx);
        let created = new_note::create_in_panel(
            &session.vault(),
            &self.notes,
            path,
            &self.focused_note(cx),
            session.tags().graph(),
            &Zoned::now(),
        );
        match created {
            Ok(landing) => self.land(&landing, Placement::Current, window, cx),
            Err(error) => self
                .sidebar
                .update(cx, |sidebar, cx| sidebar.report(&error, cx)),
        }
    }

    fn choose_template(
        &mut self,
        templates: Templates,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let tags = self.session.read(cx).tags();
        let items: Arc<[TemplateItem]> = template_items(&templates, tags.graph()).into();
        let folder = &self.notes.templates_folder;
        let empty = format!("No hay plantillas en {folder}/");
        let fuzzy = Fuzzy::new(PickerState::new(items), Haystack::Paths, empty.into());
        let options = PickerOptions {
            placeholder: "Nota nueva desde la plantilla…".into(),
            splits: false,
            focus: None,
        };
        let picker = cx.new(|cx| PickerView::new(fuzzy, options, cx));
        let events = cx.subscribe_in(&picker, window, move |this, picker, event, window, cx| {
            this.on_template_event(picker, event, &templates, window, cx);
        });
        self.show(
            Overlay::Templates {
                picker,
                _events: events,
            },
            window,
            cx,
        );
    }

    fn on_template_event(
        &mut self,
        picker: &Entity<TemplatePicker>,
        event: &PickerEvent,
        templates: &Templates,
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
                .map(|item| item.name.clone()),
        };
        self.close_overlay(window, cx);
        if let Some(template) = chosen {
            let question = self.question(Origin::Template(template), templates.clone(), cx);
            self.ask_name(question, window, cx);
        }
    }

    fn ask_name(&mut self, question: NameQuestion, window: &mut Window, cx: &mut Context<Self>) {
        let options = PickerOptions {
            placeholder: format!(
                "Nombre de la nota ({}), con / para carpetas…",
                question.origin().label()
            )
            .into(),
            splits: true,
            focus: None,
        };
        let picker = cx.new(|cx| PickerView::new(question, options, cx));
        let events = cx.subscribe_in(&picker, window, Self::on_name_event);
        self.show(
            Overlay::NoteName {
                picker,
                _events: events,
            },
            window,
            cx,
        );
    }

    fn on_name_event(
        &mut self,
        picker: &Entity<NamePicker>,
        event: &PickerEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let PickerEvent::Chosen(chosen) = event else {
            return self.close_overlay(window, cx);
        };
        let question = picker.read(cx).delegate();
        let tags = self.session.read(cx).tags();
        let prepared = question.draft(tags.graph(), &Zoned::now());
        let created = prepared
            .and_then(|(note, draft)| self.create_note(&note, &draft, chosen.open, window, cx));
        if let Err(problem) = &created {
            let reason = problem.to_string();
            picker.update(cx, |picker, cx| {
                picker.update_delegate(cx, |question, _| question.fail(reason));
            });
        }
        if created.is_ok() {
            self.close_overlay(window, cx);
            self.focus(window, cx);
        }
    }

    fn create(
        &mut self,
        question: &NameQuestion,
        open: Open,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), NameProblem> {
        let tags = self.session.read(cx).tags();
        let (note, draft) = question.draft(tags.graph(), &Zoned::now())?;
        self.create_note(&note, &draft, open, window, cx)
    }

    fn create_note(
        &mut self,
        note: &NotePath,
        draft: &Draft,
        open: Open,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), NameProblem> {
        let vault = self.session.read(cx).vault();
        let landing = new_note::create(&vault, note, draft).map_err(|error| match error {
            FileOpError::AlreadyExists(path) => NameProblem::Exists(path),
            error => NameProblem::Failed(error.to_string()),
        })?;
        let placement = match open {
            Open::Here => Placement::Current,
            Open::SplitRight => Placement::Split(Axis::Row),
            Open::SplitDown => Placement::Split(Axis::Column),
        };
        self.land(&landing, placement, window, cx);
        Ok(())
    }

    /// Abre lo creado con el cursor en su hueco y avisa de lo que la
    /// plantilla no pudo llenar.
    fn land(
        &mut self,
        landing: &Landing,
        placement: Placement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let root = self.session.read(cx).root().to_owned();
        let path = shown_path(&root.join(landing.path.as_str()));
        let opened = self.workspace.update(cx, |workspace, cx| {
            cx.notify();
            match landing.cursor {
                Some(at) => workspace.open_file_at(&path, placement, at),
                None => workspace.open_file(&path, placement),
            }
        });
        if let Err(error) = opened {
            self.workspace
                .update(cx, |workspace, _| workspace.report(&error));
        } else if let Some(problem) = landing.problems.first() {
            let more = match landing.problems.len() {
                1 => String::new(),
                count => format!(" (y {} más)", count - 1),
            };
            self.tell(
                Message::Error(format!("{}: {problem}{more}", landing.path)),
                cx,
            );
        }
        self.sync_editors(cx);
        self.focus(window, cx);
        self.touch_focused(cx);
    }
}
