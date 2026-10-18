//! `gd` y `enter` sobre un link (`editor::follow_link`): abre su nota en el
//! panel enfocado y pone el cursor en el heading o el bloque que nombra.
//! Vale para wikilinks, embeds y links markdown a una nota del vault.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use metalmind_buffer::{ByteOffset, Selection, Selections};
use metalmind_vault::{LinkIndex, LinkSubpath, LinkTarget, VaultPath};

use super::{Effect, Message, Workspace, WorkspaceError, clamp_cursor, shown_path};
use crate::finder::relative_to;

/// Los links del vault abierto, como los publica su watcher.
#[derive(Clone, Debug)]
pub(super) struct VaultLinks {
    /// Sin enlaces simbólicos.
    root: PathBuf,
    index: Arc<LinkIndex>,
}

/// Un link que empata entre varias notas, esperando que se elija una.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkChoice {
    pub candidates: Vec<VaultPath>,
    subpath: Option<LinkSubpath>,
}

impl Workspace {
    /// El índice de links del vault de raíz `root`, sin enlaces simbólicos,
    /// con el que siguen los links `gd` y `enter`.
    pub fn set_links(&mut self, root: &Path, index: Arc<LinkIndex>) {
        self.links = Some(VaultLinks {
            root: root.to_owned(),
            index,
        });
    }

    /// Las notas entre las que elegir tras [`Effect::ChooseLink`].
    #[must_use]
    pub fn link_choice(&self) -> Option<&LinkChoice> {
        self.link_choice.as_ref()
    }

    /// Abre la candidata `chosen` de [`Workspace::link_choice`]; con `None`
    /// se descarta la elección.
    ///
    /// # Errors
    /// Solo si el estado está inconsistente, lo que sería un bug.
    pub fn choose_link(&mut self, chosen: Option<usize>) -> Result<(), WorkspaceError> {
        let Some(choice) = self.link_choice.take() else {
            return Ok(());
        };
        let (Some(links), Some(path)) =
            (&self.links, chosen.and_then(|i| choice.candidates.get(i)))
        else {
            return Ok(());
        };
        let path = links.root.join(path.as_str());
        self.open_link(&path, choice.subpath.as_ref())
    }

    /// `editor::follow_link`: sigue el link bajo el cursor del panel
    /// enfocado. Si no hay link o no lleva a ninguna nota, lo avisa y no
    /// hace nada; si empata, pide elegir.
    pub(super) fn follow_link(&mut self) -> Result<Effect, WorkspaceError> {
        let Some(links) = self.links.clone() else {
            return Ok(
                self.not_followed("no hay un vault abierto: los links no llevan a ningún lado")
            );
        };
        let view = self.view(self.focused())?;
        let source = view
            .document
            .resolved_path()
            .and_then(|path| relative_to(&links.root, &path));
        let Some(source) = source else {
            return Ok(
                self.not_followed("la nota no está en el vault: sus links no llevan a ningún lado")
            );
        };
        let text = view.buffer().to_string();
        let Some(destination) = links.index.follow(&source, &text, view.cursor().get()) else {
            return Ok(self.not_followed("no hay un link en el cursor"));
        };
        let subpath = destination.subpath;
        match destination.target {
            LinkTarget::SameNote => {
                if let Some(subpath) = &subpath {
                    self.push_jump(self.focused())?;
                    self.go_to_subpath(subpath)?;
                }
                Ok(Effect::Stay)
            }
            LinkTarget::Resolved(path) => {
                self.open_link(&links.root.join(path.as_str()), subpath.as_ref())?;
                Ok(Effect::Stay)
            }
            LinkTarget::Ambiguous(candidates) => {
                self.link_choice = Some(LinkChoice {
                    candidates,
                    subpath,
                });
                Ok(Effect::ChooseLink)
            }
            LinkTarget::Unresolved => Ok(self.not_followed(&format!(
                "el link no lleva a ninguna nota: {}",
                destination.target_text
            ))),
        }
    }

    /// Abre `path` en el panel enfocado como un salto, con el cursor en
    /// `subpath` si lo trae y está en la nota.
    fn open_link(
        &mut self,
        path: &Path,
        subpath: Option<&LinkSubpath>,
    ) -> Result<(), WorkspaceError> {
        let pane = self.focused();
        self.push_jump(pane)?;
        if self.show_file(pane, &shown_path(path))?.is_some()
            && let Some(subpath) = subpath
        {
            self.go_to_subpath(subpath)?;
        }
        Ok(())
    }

    /// El cursor del panel enfocado va al inicio del heading o del bloque.
    fn go_to_subpath(&mut self, subpath: &LinkSubpath) -> Result<(), WorkspaceError> {
        let text = self.focused_document()?.buffer().to_string();
        let Some(found) = subpath.locate(&text) else {
            let name = match subpath {
                LinkSubpath::Heading(heading) => format!("#{heading}"),
                LinkSubpath::Block(id) => format!("#^{id}"),
            };
            self.message = Some(Message::Error(format!("la nota no tiene {name}")));
            return Ok(());
        };
        self.edit_pane(self.focused(), |vim, buffer, selections| {
            vim.cursor_moved();
            let head = clamp_cursor(buffer, ByteOffset::new(found.start), vim.mode());
            *selections = Selections::single(Selection::cursor(head));
            Ok(())
        })
    }

    fn not_followed(&mut self, why: &str) -> Effect {
        self.message = Some(Message::Error(why.to_owned()));
        Effect::Stay
    }
}

#[cfg(test)]
#[path = "links_tests.rs"]
pub(super) mod tests;
