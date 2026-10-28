//! Las listas de `:`, que van en su pop-up: la del historial y el nombre
//! del comando, y la del argumento (#156), con lo que ofrece cada comando.

use std::sync::Arc;

use gpui_kit::{App, Context};
use metalmind_keymap::Command;
use metalmind_vault::VaultIndex;
use metalmind_vim::{ActionName, TypedArgument};

use super::super::Editor;
use super::super::command_popup::PopupList;
use super::{Completion, List, rows};
use crate::completion::{ArgumentCompletions, ArgumentSources, FolderCandidates};
use crate::theme::Theme;

const FOCUS: ActionName = ActionName::new(Command::Focus.name());

impl Completion {
    fn folder_candidates(&mut self, index: Arc<VaultIndex>) -> Arc<FolderCandidates> {
        match &self.folders {
            Some(cached) if cached.is_of(&index) => Arc::clone(cached),
            _ => {
                let candidates = Arc::new(FolderCandidates::new(index));
                self.folders = Some(Arc::clone(&candidates));
                candidates
            }
        }
    }
}

impl Editor {
    /// La lista de lo que se escribe tras `:comando `, en los comandos que
    /// completan su argumento.
    pub(super) fn argument_list(&mut self, typed: &TypedArgument, cx: &App) -> Option<List> {
        if typed.action != FOCUS {
            return None;
        }
        let session = self.session.read(cx);
        let (tags, index) = (session.tags(), session.vault().snapshot());
        let completion = &mut self.completion;
        let sources = ArgumentSources {
            tags: completion.tag_candidates(tags),
            folders: completion.folder_candidates(index),
            names: &mut completion.names,
            paths: &mut completion.paths,
        };
        let previous = match completion.list.take() {
            Some(List::Argument(list)) => Some(list),
            _ => None,
        };
        ArgumentCompletions::focus(&typed.word, sources, previous).map(List::Argument)
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
