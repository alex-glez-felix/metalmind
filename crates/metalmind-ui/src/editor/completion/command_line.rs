//! Las listas de `:`, que van en su pop-up: la del historial y el nombre
//! del comando, y la del argumento (#156, #45), con lo que ofrece cada
//! comando.

use std::sync::Arc;

use gpui_kit::{App, Context};
use metalmind_keymap::Command;
use metalmind_vault::VaultIndex;
use metalmind_vim::{ActionName, TypedArgument};

use super::super::Editor;
use super::super::command_popup::PopupList;
use super::{Completion, List, rows};
use crate::completion::{ArgumentCompletions, FocusSources, PathCandidates};
use crate::theme::Theme;

const FOCUS: ActionName = ActionName::new(Command::Focus.name());
const EDIT: ActionName = ActionName::new(Command::Edit.name());
const FORCE_EDIT: ActionName = ActionName::new(Command::ForceEdit.name());

impl Completion {
    fn folder_candidates(&mut self, index: Arc<VaultIndex>) -> Arc<PathCandidates> {
        cached(&mut self.folders, index, PathCandidates::folders)
    }

    fn file_candidates(&mut self, index: Arc<VaultIndex>) -> Arc<PathCandidates> {
        cached(&mut self.files, index, PathCandidates::files)
    }
}

/// Las rutas de `index` guardadas en `cache`, armadas de nuevo con `build`
/// solo si llegó otro índice.
fn cached(
    cache: &mut Option<Arc<PathCandidates>>,
    index: Arc<VaultIndex>,
    build: fn(Arc<VaultIndex>) -> PathCandidates,
) -> Arc<PathCandidates> {
    match cache {
        Some(cached) if cached.is_of(&index) => Arc::clone(cached),
        _ => Arc::clone(cache.insert(Arc::new(build(index)))),
    }
}

impl Editor {
    /// La lista de lo que se escribe tras `:comando `, en los comandos que
    /// completan su argumento.
    pub(super) fn argument_list(&mut self, typed: &TypedArgument, cx: &App) -> Option<List> {
        let session = self.session.read(cx);
        let (tags, index) = (session.tags(), session.vault().snapshot());
        let completion = &mut self.completion;
        let previous = match completion.list.take() {
            Some(List::Argument(list)) => Some(list),
            _ => None,
        };
        let list = if typed.action == FOCUS {
            let sources = FocusSources {
                tags: completion.tag_candidates(tags),
                folders: completion.folder_candidates(index),
                names: &mut completion.names,
                paths: &mut completion.paths,
            };
            ArgumentCompletions::focus(&typed.word, sources, previous)
        } else if typed.action == EDIT || typed.action == FORCE_EDIT {
            let files = completion.file_candidates(index);
            ArgumentCompletions::edit(&typed.word, files, &mut completion.paths, previous)
        } else {
            None
        };
        list.map(List::Argument)
    }

    /// La lista abierta de `:`, con las filas que caben en el pop-up.
    pub(in crate::editor) fn command_line_list(
        &mut self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<PopupList> {
        let list = self.completion.list.as_mut()?;
        let view = list.view();
        let wanted = usize::from(theme.layout.picker_rows).min(list.len());
        let visible = list.selection_mut().visible(wanted);
        let rows = match &*list {
            List::Command(list) => rows::commands(list, visible.clone(), theme),
            List::Argument(list) => rows::argument(list, visible.clone(), theme),
            List::Tags(_) | List::Links(_) => return None,
        };
        Some(PopupList {
            rows,
            visible,
            selected: list.selection().selected(),
            accept_keys: Self::accept_keys(view, cx),
        })
    }
}
