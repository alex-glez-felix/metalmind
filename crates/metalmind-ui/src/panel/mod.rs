//! El panel lateral sin GPUI: si se ve, en qué vista, el árbol de carpetas,
//! el de tags, su selección y su focus. Lo que le pregunta al usuario vive en
//! [`prompt`].

mod marks;
mod outline;
mod prompt;

use std::sync::Arc;

use metalmind_config::SavedView;
use metalmind_vault::tags::{Scope, Tags};
use metalmind_vault::{VaultChange, VaultPath};

pub use self::marks::{Fill, RowMarks};
pub use self::outline::Outline;
pub use self::prompt::{NewEntry, Prompt, Request};
use crate::file_tree::FileTree;
use crate::focus::{Focus, FocusError, InScope};
use crate::tag_outline::TagOutline;

/// Las vistas del panel: carpetas y tags (05 §3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PanelView {
    #[default]
    Folders,
    Tags,
}

impl PanelView {
    /// La que sigue con `T`, y tras la última la primera.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Folders => Self::Tags,
            Self::Tags => Self::Folders,
        }
    }
}

/// Lo que deja mostrar una vista.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shown {
    pub visible: bool,
    /// La fila elegida de tags, si rehacer la vista al mostrarla la cambió
    /// de lugar: hay que llevarla a la vista.
    pub moved: Option<usize>,
}

/// Un archivo que un clic pide abrir.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClickOpen {
    pub path: VaultPath,
    /// El doble clic le pasa el foco al editor; el simple lo deja en el
    /// árbol.
    pub focus_editor: bool,
}

#[derive(Clone, Debug)]
pub struct Panel {
    pub visible: bool,
    pub view: PanelView,
    pub tree: FileTree,
    pub tags: TagOutline,
    prompt: Option<Prompt>,
    /// El último error de una operación, hasta la próxima.
    error: Option<String>,
    /// La carpeta del focus: lo que se crea va dentro de ella.
    focus_folder: Option<VaultPath>,
}

impl Panel {
    #[must_use]
    pub fn new(tree: FileTree, tags: TagOutline, visible: bool, view: PanelView) -> Self {
        let mut panel = Self {
            visible,
            view,
            tree,
            tags,
            prompt: None,
            error: None,
            focus_folder: None,
        };
        panel.refresh_shown();
        panel
    }

    #[must_use]
    pub fn prompt(&self) -> Option<&Prompt> {
        self.prompt.as_ref()
    }

    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn report(&mut self, error: &impl std::fmt::Display) {
        self.error = Some(error.to_string());
    }

    /// Muestra `view`, o esconde el panel si ya la mostraba con el foco
    /// (`focused`): si la mostraba sin el foco, sigue visible para que lo
    /// tome, como `ToggleFocus` de Zed.
    pub fn toggle(&mut self, view: PanelView, focused: bool) -> Shown {
        self.visible = !(self.visible && self.view == view && focused);
        if self.view != view {
            self.show(view);
        }
        Shown {
            visible: self.visible,
            moved: self.refresh_shown(),
        }
    }

    /// `T`: la vista que sigue. Devuelve la fila elegida de tags si al
    /// rehacerla cambió de lugar.
    pub fn switch_view(&mut self) -> Option<usize> {
        self.show(self.view.next());
        self.refresh_shown()
    }

    /// Cambia de vista: lo que se estaba preguntando era sobre la otra. Un
    /// filtro a medio escribir se queda puesto, como con `Enter`.
    fn show(&mut self, view: PanelView) {
        self.prompt = None;
        self.view = view;
    }

    /// Los tags de otro lote del watcher; la vista de tags se rehace solo
    /// si se ve.
    pub fn set_tags(&mut self, tags: Arc<Tags>) {
        self.tags.set_tags(tags);
        self.refresh_shown();
    }

    /// Las vistas guardadas del vault, arriba en la vista de tags.
    pub fn set_views(&mut self, views: Arc<[SavedView]>) {
        self.tags.set_views(views);
        self.refresh_shown();
    }

    /// Rehace la vista de tags si se ve; devuelve su fila elegida si con eso
    /// cambió de lugar.
    fn refresh_shown(&mut self) -> Option<usize> {
        if !(self.visible && self.view == PanelView::Tags) {
            return None;
        }
        let before = self.tags.selected_row();
        self.tags.refresh();
        let after = self.tags.selected_row();
        after.filter(|_| after != before)
    }

    /// La vista que se ve.
    #[must_use]
    pub fn outline(&self) -> &dyn Outline {
        match self.view {
            PanelView::Folders => &self.tree,
            PanelView::Tags => &self.tags,
        }
    }

    pub fn outline_mut(&mut self) -> &mut dyn Outline {
        match self.view {
            PanelView::Folders => &mut self.tree,
            PanelView::Tags => &mut self.tags,
        }
    }

    /// Un clic en la fila `at` de la vista que se ve, el `count`-ésimo
    /// seguido: selecciona, y el primero abre o cierra una carpeta o un tag.
    /// Sobre un archivo, cada clic lo abre, como en Zed: el primero deja el
    /// foco en el panel y el doble se lo pasa al editor. El segundo clic
    /// sobre una carpeta no hace nada: si no, un doble clic la dejaría como
    /// estaba.
    pub fn click(&mut self, at: usize, count: usize) -> Option<ClickOpen> {
        let outline = self.outline_mut();
        outline.select_row(at);
        if let Some(path) = outline.selected_file() {
            return Some(ClickOpen {
                path: path.clone(),
                focus_editor: count > 1,
            });
        }
        if count == 1 {
            outline.toggle_selected();
        }
        None
    }

    /// Otro focus, o el mismo con otro índice: las dos vistas lo siguen.
    pub fn set_focus(&mut self, scope: Scope, in_scope: Option<Arc<InScope>>) {
        self.focus_folder.clone_from(&scope.folder);
        self.tree.set_scope(in_scope);
        self.tags.set_scope(scope);
        self.refresh_shown();
    }

    /// El focus de la fila elegida: una carpeta, un tag o una vista. Un
    /// archivo no es un alcance; una vista con algo que no es de tags es un
    /// error.
    #[must_use]
    pub fn selected_focus(&self) -> Option<Result<Focus, FocusError>> {
        match self.view {
            PanelView::Folders => self
                .tree
                .selected_entry()
                .filter(|row| row.is_folder())
                .map(|row| Ok(Focus::folder(row.path.clone()))),
            PanelView::Tags => match self.selected_view() {
                Some(view) => Some(Focus::view(view)),
                None => self.selected_tag().as_deref().map(Focus::tag).map(Ok),
            },
        }
    }

    /// La vista guardada de la fila elegida, si se ve la vista de tags.
    #[must_use]
    pub fn selected_view(&self) -> Option<&SavedView> {
        if self.view != PanelView::Tags {
            return None;
        }
        self.tags.selected_entry()?.view()
    }

    /// La forma establecida del tag de la fila elegida en la vista de tags.
    #[must_use]
    pub fn selected_tag(&self) -> Option<String> {
        self.tags
            .selected_entry()
            .and_then(|row| row.tag_name())
            .map(str::to_owned)
    }

    /// Tras un lote del watcher, antes de sus tags: la nota seleccionada en
    /// la vista de tags y la pregunta abierta siguen a su entrada en los
    /// renombres y, si la entrada de la pregunta desaparece, se cierra con
    /// un aviso. Así confirmar nunca actúa sobre otra entrada que ocupe la
    /// ruta vieja.
    pub fn follow(&mut self, changes: &[VaultChange]) {
        self.tags.follow(changes);
        let Some(target) = self.prompt.as_mut().and_then(Prompt::target_mut) else {
            return;
        };
        for change in changes {
            match change {
                VaultChange::Renamed { from, to, .. } => {
                    if let Some(moved) = target.rebase(from, to) {
                        *target = moved;
                    }
                }
                VaultChange::Removed { path, .. } if target.starts_with(path) => {
                    let gone = target.clone();
                    self.prompt = None;
                    self.error = Some(format!("«{gone}» ya no existe"));
                    return;
                }
                _ => {}
            }
        }
    }

    /// `Escape` sin menú: cierra la pregunta o, si no hay y se ve el árbol
    /// de carpetas, quita su filtro y luego su selección, una cosa a la vez.
    pub fn escape(&mut self) {
        if self.prompt.is_some() {
            self.cancel();
        } else if self.view == PanelView::Folders {
            if self.tree.is_filtering() {
                self.tree.set_filter("");
            } else {
                self.tree.clear_selection();
            }
        }
    }
}

#[cfg(test)]
mod focus_tests;
#[cfg(test)]
mod tag_field_tests;
#[cfg(test)]
mod tests;
