//! La lista del argumento de `:` (#156): qué ofrece cada comando y dónde
//! va, sobre la barra de estado, al principio de la línea de comandos.

use std::sync::Arc;

use gpui_kit::{
    Anchor, AnchoredPositionMode, AnyElement, App, Context, IntoElement, ParentElement as _,
    Pixels, anchored, deferred, point, px,
};
use metalmind_keymap::Command;
use metalmind_vault::VaultIndex;
use metalmind_vim::{ActionName, TypedArgument};

use super::super::Editor;
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

    /// La lista del argumento, que crece hacia arriba desde el borde de la
    /// barra de estado. No pasa de `max_width`, el ancho de la ventana.
    pub(in crate::editor) fn command_line_menu(
        &mut self,
        max_width: Pixels,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let Some(List::Argument(list)) = &mut self.completion.list else {
            return None;
        };
        let wanted = usize::from(theme.layout.completion_rows).min(list.len());
        let visible = list.selection_mut().visible(wanted);
        let rows = rows::argument(list, visible.clone(), theme);
        let selected = list.selection().selected();
        let menu = Self::menu(rows, visible, selected, max_width, theme, cx);
        let above_bar = -(theme.layout.status_padding_y + theme.stroke.border);
        Some(
            deferred(
                anchored()
                    .anchor(Anchor::BottomLeft)
                    .position_mode(AnchoredPositionMode::Local)
                    .position(point(px(0.), above_bar))
                    .snap_to_window()
                    .child(menu),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }
}
