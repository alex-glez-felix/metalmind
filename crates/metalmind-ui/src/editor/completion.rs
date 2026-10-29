//! Las listas del autocompletado en el editor: la de `#` (05 §3) se abre
//! al escribir un tag y cambia lo escrito por su forma establecida; la de
//! `[[`, al escribir el destino de un wikilink, y escribe con qué nombre
//! llega el link a la nota elegida. Las dos se pintan bajo lo escrito (o
//! encima, si no cabe) y se aceptan con `tab` o un clic. Las de `:` (el
//! historial, el nombre del comando y su argumento, #156) van en el pop-up
//! de la línea de comandos (ver `command_popup`).

use std::collections::BTreeSet;
use std::sync::Arc;

use gpui_kit::{App, Context, KeyContext, Window};
use metalmind_keymap::{View, context};
use metalmind_search::FuzzyMatcher;
use metalmind_syntax::TagPlace;
use metalmind_vault::VaultPath;
use metalmind_vault::tags::{TagCompletion, Tags};
use metalmind_vim::{Mode, TypedArgument, TypedCommand};

use super::Editor;
use crate::actions::EDITOR_CONTEXT;
use crate::clipboard::SystemClipboard;
use crate::completion::{
    ArgumentCompletions, CommandCompletions, FolderCandidates, LinkCompletions, NoteCandidates,
    Selection, TagCompletions,
};
use crate::finder::Note;
use crate::workspace::{Message, PaneId, TypedLink, TypedTag, Workspace, WorkspaceError};

mod command_line;
mod overlay;
mod rows;

/// La lista abierta.
enum List {
    Tags(TagCompletions),
    Links(LinkCompletions),
    /// El historial o el nombre de un comando de `:`.
    Command(CommandCompletions),
    Argument(ArgumentCompletions),
}

impl List {
    fn selection(&self) -> &Selection {
        match self {
            Self::Tags(list) => list.selection(),
            Self::Links(list) => list.selection(),
            Self::Command(list) => list.selection(),
            Self::Argument(list) => list.selection(),
        }
    }

    fn selection_mut(&mut self) -> &mut Selection {
        match self {
            Self::Tags(list) => list.selection_mut(),
            Self::Links(list) => list.selection_mut(),
            Self::Command(list) => list.selection_mut(),
            Self::Argument(list) => list.selection_mut(),
        }
    }

    /// Lo escrito bajo lo que va la lista; las de `:` van en su pop-up.
    fn typed_in_text(&self) -> Option<&str> {
        match self {
            Self::Tags(list) => Some(list.typed()),
            Self::Links(list) => Some(list.typed()),
            Self::Command(_) | Self::Argument(_) => None,
        }
    }

    fn len(&self) -> usize {
        match self {
            Self::Tags(list) => list.rows().len(),
            Self::Links(list) => list.rows().len(),
            Self::Command(list) => list.len(),
            Self::Argument(list) => list.len(),
        }
    }

    /// Si es la lista de lo que se escribe ahora, aunque con otro texto.
    fn is_for(&self, typed: &Typed) -> bool {
        matches!(
            (self, typed),
            (Self::Tags(_), Typed::Tag(_))
                | (Self::Links(_), Typed::Link(_))
                | (Self::Command(_), Typed::Command(_))
                | (Self::Argument(_), Typed::Argument(_))
        )
    }

    /// Dónde valen sus atajos: en las de `:` fuera de un argumento, las
    /// flechas son del historial.
    fn view(&self) -> View {
        match self {
            Self::Command(_) => View::CommandCompletion,
            Self::Tags(_) | Self::Links(_) | Self::Argument(_) => View::Completion,
        }
    }
}

/// Lo que el editor guarda para el autocompletado.
pub(super) struct Completion {
    list: Option<List>,
    /// Los candidatos de unos tags: se arman de nuevo solo si llegan otros.
    tags: Option<(Arc<Tags>, Arc<[TagCompletion]>)>,
    /// Las notas de `[[`: se arman de nuevo solo si cambian.
    notes: Option<Arc<NoteCandidates>>,
    /// Las carpetas de un índice del vault, para `:focus`.
    folders: Option<Arc<FolderCandidates>>,
    /// Para nombres de tags.
    names: FuzzyMatcher,
    /// Para rutas de notas.
    paths: FuzzyMatcher,
}

impl Default for Completion {
    fn default() -> Self {
        Self {
            list: None,
            tags: None,
            notes: None,
            folders: None,
            names: FuzzyMatcher::text(),
            paths: FuzzyMatcher::paths(),
        }
    }
}

impl Completion {
    pub(super) fn is_open(&self) -> bool {
        self.list.is_some()
    }

    pub(super) fn close(&mut self) {
        self.list = None;
    }

    fn tag_candidates(&mut self, tags: Arc<Tags>) -> Arc<[TagCompletion]> {
        match &self.tags {
            Some((cached, candidates)) if Arc::ptr_eq(cached, &tags) => Arc::clone(candidates),
            _ => {
                let candidates: Arc<[TagCompletion]> = tags.completions().into();
                self.tags = Some((tags, Arc::clone(&candidates)));
                candidates
            }
        }
    }
}

impl Completion {
    /// Un lote del vault trae otra lista de notas aunque no cambie (un
    /// autoguardado): convertirlas para filtrar se hace solo si cambió.
    fn note_candidates(&mut self, notes: Arc<[Note]>) -> Arc<NoteCandidates> {
        match &self.notes {
            Some(cached) if cached.has(&notes) => Arc::clone(cached),
            _ => {
                let candidates = Arc::new(NoteCandidates::new(notes));
                self.notes = Some(Arc::clone(&candidates));
                candidates
            }
        }
    }
}

impl Editor {
    /// El del editor y, con una lista abierta, el de sus atajos.
    pub(super) fn key_context(&self) -> KeyContext {
        let mut key_context = KeyContext::new_with_defaults();
        key_context.add(EDITOR_CONTEXT);
        match self.completion.list.as_ref().map(List::view) {
            Some(View::CommandCompletion) => key_context.add(context::COMMAND_COMPLETION),
            Some(_) => key_context.add(context::COMPLETION),
            None => {}
        }
        key_context
    }

    /// Tras cualquier cambio del workspace o del vault, la lista abierta
    /// sigue a lo que se está escribiendo con los tags y las notas de
    /// ahora, o se cierra. La de `:` fuera de un argumento también se abre:
    /// al abrir la línea, tras recorrer el historial o tras borrar.
    pub(super) fn refresh_completion(&mut self, cx: &App) {
        if !self.completion.is_open() && self.mode(cx) != Mode::CommandLine {
            return;
        }
        let typed = self.typed(cx);
        let follows = match (&self.completion.list, &typed) {
            (_, Some(Typed::Command(_))) => true,
            (Some(list), Some(typed)) => list.is_for(typed),
            _ => false,
        };
        self.completion.list = if follows {
            self.list_for(typed, cx)
        } else {
            None
        };
    }

    /// Tras escribir texto: abre, filtra o cierra la lista según lo que se
    /// está escribiendo. Si lo escrito no cambió, la fila que eligió el
    /// usuario sigue elegida.
    pub(super) fn update_completion(&mut self, cx: &App) {
        let typed = self.typed(cx);
        self.completion.list = self.list_for(typed, cx);
    }

    fn typed(&self, cx: &App) -> Option<Typed> {
        Typed::in_pane(self.workspace.read(cx), self.pane)
            .inspect_err(|error| log::error!("{error}"))
            .ok()
            .flatten()
    }

    fn list_for(&mut self, typed: Option<Typed>, cx: &App) -> Option<List> {
        match typed? {
            Typed::Link(link) => self.link_list(&link, cx),
            Typed::Tag(tag) => self.tag_list(&tag, cx),
            Typed::Command(command) => {
                let previous = match self.completion.list.take() {
                    Some(List::Command(list)) => Some(list),
                    _ => None,
                };
                Some(List::Command(CommandCompletions::open(command, previous)))
            }
            Typed::Argument(argument) => self.argument_list(&argument, cx),
        }
    }

    /// La lista de `[[`: solo en una nota del vault, que es desde donde se
    /// calcula cómo nombrar a las demás.
    fn link_list(&mut self, typed: &TypedLink, cx: &App) -> Option<List> {
        let source = self.note_path(cx)?;
        let notes = self.session.read(cx).notes();
        let candidates = self.completion.note_candidates(notes);
        let matcher = &mut self.completion.paths;
        match self.completion.list.take() {
            Some(List::Links(list)) if list.is_for(&candidates, &typed.typed, &source) => {
                Some(list)
            }
            Some(List::Links(list)) if list.is_among(&candidates, &source) => {
                list.retyped(&typed.typed, matcher)
            }
            Some(List::Links(list)) if list.typed() == typed.typed && *list.source() == source => {
                list.with_candidates(candidates, matcher)
            }
            _ => LinkCompletions::open(candidates, &typed.typed, &source, matcher),
        }
        .map(List::Links)
    }

    /// La lista de `#`. `aliases` y `parents` solo nombran tags en una
    /// página de tag.
    fn tag_list(&mut self, typed: &TypedTag, cx: &App) -> Option<List> {
        let tags = self.session.read(cx).tags();
        let note = self.note_path(cx);
        if matches!(typed.place, TagPlace::Aliases | TagPlace::Parents)
            && !note.as_ref().is_some_and(|path| tags.is_page(path))
        {
            return None;
        }
        let hidden = match &note {
            Some(note) => tags.unwritten_in(note, || self.written(cx), &typed.range),
            None => BTreeSet::new(),
        };
        let candidates = self.completion.tag_candidates(tags);
        let matcher = &mut self.completion.names;
        match self.completion.list.take() {
            Some(List::Tags(list)) if list.is_for(&candidates, &typed.typed, typed.place) => {
                Some(list)
            }
            Some(List::Tags(list)) if list.is_typing(&typed.typed, typed.place) => {
                list.with_candidates(candidates, &hidden, matcher)
            }
            _ => TagCompletions::open(candidates, &typed.typed, typed.place, &hidden, matcher),
        }
        .map(List::Tags)
    }

    /// El texto del panel como está ahora, guardado o no.
    fn written(&self, cx: &App) -> String {
        self.view(cx)
            .map(|view| view.buffer().to_string())
            .unwrap_or_default()
    }

    /// Escribe lo elegido y cierra la lista: lo que queda escrito ya es el
    /// tag o el link entero.
    pub(super) fn accept_completion(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(list) = self.completion.list.take() else {
            return;
        };
        // Como una tecla del motor: lo que componía el IME ya no aplica.
        self.marked = None;
        let result = match list {
            List::Tags(list) => {
                let text = list.accept();
                self.update_workspace(cx, |workspace, pane, cx| {
                    workspace.complete_tag(pane, &text, &mut SystemClipboard(cx))
                })
            }
            List::Links(list) => {
                if let Some(target) = self.wikilink_target(list.accept(), cx) {
                    self.update_workspace(cx, |workspace, pane, cx| {
                        workspace.complete_link(pane, &target, &mut SystemClipboard(cx))
                    })
                } else {
                    let message = Message::unnameable(list.accept().as_str());
                    self.update_workspace(cx, |workspace, _, _| workspace.show_message(message));
                    Ok(())
                }
            }
            List::Command(list) => {
                let text = list.accept();
                self.update_workspace(cx, |workspace, pane, _| {
                    workspace.complete_command(pane, &text);
                    Ok(())
                })
            }
            List::Argument(list) => {
                let text = list.accept();
                self.update_workspace(cx, |workspace, pane, _| {
                    workspace.complete_argument(pane, &text);
                    Ok(())
                })
            }
        };
        if let Err(error) = result {
            self.report(&error, cx);
        }
        self.synced = None;
        window.refresh();
        cx.notify();
    }

    /// Con qué nombre llega a `target` un wikilink escrito en esta nota.
    fn wikilink_target(&mut self, target: &VaultPath, cx: &App) -> Option<String> {
        let source = self.note_path(cx)?;
        let links = self.session.read(cx).vault().links();
        links.wikilink_target(&source, target)
    }

    pub(super) fn select_next_completion(&mut self, cx: &mut Context<Self>) {
        if let Some(list) = &mut self.completion.list {
            list.selection_mut().select_next();
            cx.notify();
        }
    }

    pub(super) fn select_previous_completion(&mut self, cx: &mut Context<Self>) {
        if let Some(list) = &mut self.completion.list {
            list.selection_mut().select_previous();
            cx.notify();
        }
    }
}

/// Lo que se está escribiendo.
enum Typed {
    Link(TypedLink),
    Tag(TypedTag),
    Command(TypedCommand),
    Argument(TypedArgument),
}

impl Typed {
    fn in_pane(workspace: &Workspace, pane: PaneId) -> Result<Option<Self>, WorkspaceError> {
        if let Some(argument) = workspace.typed_argument(pane) {
            return Ok(Some(Self::Argument(argument)));
        }
        if let Some(command) = workspace.typed_command(pane) {
            return Ok(Some(Self::Command(command)));
        }
        if let Some(link) = workspace.typed_link(pane)? {
            return Ok(Some(Self::Link(link)));
        }
        Ok(workspace.typed_tag(pane)?.map(Self::Tag))
    }
}
