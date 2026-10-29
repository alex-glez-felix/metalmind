//! El panel lateral sin GPUI: si se ve, en qué vista, el árbol de carpetas,
//! el de tags y lo que se le está preguntando al usuario (un nombre, el filtro o
//! confirmar un borrado). Confirmar devuelve un [`Request`] que ejecuta
//! quien tiene el vault; así las reglas de los nombres se prueban sin disco.

mod marks;
mod outline;

use std::sync::Arc;

use metalmind_syntax::note_name;
use metalmind_vault::tags::{Scope, Tags, tag_name};
use metalmind_vault::{EntryKind, InvalidPath, VaultChange, VaultPath};

pub use self::marks::{Fill, RowMarks};
pub use self::outline::Outline;
use crate::backlinks::BacklinkOutline;
use crate::file_tree::FileTree;
use crate::focus::{Focus, InScope};
use crate::picker::InputLine;
use crate::tag_change::{FieldError, TagField, TagRequest};
use crate::tag_outline::TagOutline;
use crate::workspace::LinePosition;

/// Las vistas del panel: carpetas, tags (05 §3) y los backlinks de la
/// nota activa.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PanelView {
    #[default]
    Folders,
    Tags,
    Backlinks,
}

impl PanelView {
    /// La que sigue con `T`, y tras la última la primera.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Folders => Self::Tags,
            Self::Tags => Self::Backlinks,
            Self::Backlinks => Self::Folders,
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
    /// Dónde, si la fila lo dice (un backlink).
    pub at: Option<LinePosition>,
    /// El doble clic le pasa el foco al editor; el simple lo deja en el
    /// árbol.
    pub focus_editor: bool,
}

/// Qué se crea con [`Panel::start_create`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewEntry {
    Note,
    Folder,
}

/// Lo que el panel está preguntando.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Prompt {
    Filter(InputLine),
    Create {
        parent: VaultPath,
        kind: NewEntry,
        input: InputLine,
    },
    Rename {
        path: VaultPath,
        input: InputLine,
    },
    /// Como renombrar, pero se escribe la ruta entera.
    Move {
        path: VaultPath,
        input: InputLine,
    },
    ConfirmDelete {
        path: VaultPath,
        /// Algún buffer de lo que se borra tiene cambios sin guardar.
        unsaved: bool,
    },
    /// El nombre nuevo del tag elegido en la vista de tags.
    RenameTag {
        tag: String,
        input: InputLine,
    },
    /// El tag con que se fusiona el elegido.
    MergeTag {
        tag: String,
        input: InputLine,
    },
    /// Un campo de la página del tag elegido, con lo que tiene ahora.
    TagField {
        tag: String,
        field: TagField,
        input: InputLine,
    },
}

impl Prompt {
    /// La vista que pregunta esto.
    fn view(&self) -> PanelView {
        match self {
            Self::RenameTag { .. } | Self::MergeTag { .. } | Self::TagField { .. } => {
                PanelView::Tags
            }
            _ => PanelView::Folders,
        }
    }

    /// La entrada sobre la que actúa: la que se renombra, mueve o borra, o
    /// la carpeta donde se crea. El filtro no tiene.
    fn target_mut(&mut self) -> Option<&mut VaultPath> {
        match self {
            Self::Filter(_)
            | Self::RenameTag { .. }
            | Self::MergeTag { .. }
            | Self::TagField { .. } => None,
            Self::Create { parent, .. } => Some(parent),
            Self::Rename { path, .. }
            | Self::Move { path, .. }
            | Self::ConfirmDelete { path, .. } => Some(path),
        }
    }
}

/// Una operación de archivo que el usuario confirmó.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    CreateNote(VaultPath),
    CreateFolder(VaultPath),
    Rename {
        from: VaultPath,
        to: VaultPath,
    },
    Trash(VaultPath),
    /// Renombrar o fusionar un tag: la ventana muestra su vista previa.
    ChangeTags(TagRequest),
}

#[derive(Clone, Debug)]
pub struct Panel {
    pub visible: bool,
    pub view: PanelView,
    pub tree: FileTree,
    pub tags: TagOutline,
    pub backlinks: BacklinkOutline,
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
            backlinks: BacklinkOutline::new(Arc::default()),
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
            PanelView::Backlinks => &self.backlinks,
        }
    }

    pub fn outline_mut(&mut self) -> &mut dyn Outline {
        match self.view {
            PanelView::Folders => &mut self.tree,
            PanelView::Tags => &mut self.tags,
            PanelView::Backlinks => &mut self.backlinks,
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
                at: outline.selected_position(),
                focus_editor: count > 1,
            });
        }
        if count == 1 {
            outline.toggle_selected();
        }
        None
    }

    /// El campo que se está editando, si hay uno.
    pub fn input_mut(&mut self) -> Option<&mut InputLine> {
        match self.prompt.as_mut()? {
            Prompt::Filter(input)
            | Prompt::Create { input, .. }
            | Prompt::Rename { input, .. }
            | Prompt::Move { input, .. }
            | Prompt::RenameTag { input, .. }
            | Prompt::MergeTag { input, .. }
            | Prompt::TagField { input, .. } => Some(input),
            Prompt::ConfirmDelete { .. } => None,
        }
    }

    /// Tras editar el campo: el filtro se aplica mientras se escribe.
    pub fn input_changed(&mut self) {
        if let Some(Prompt::Filter(input)) = &self.prompt {
            let query = input.text().to_owned();
            self.tree.set_filter(&query);
        }
    }

    pub fn start_filter(&mut self) {
        self.open(Prompt::Filter(InputLine::new(self.tree.filter())));
    }

    /// Dentro de la carpeta seleccionada, o junto al archivo seleccionado;
    /// en focus de carpeta, nunca fuera de ella.
    pub fn start_create(&mut self, kind: NewEntry) {
        let selected = match self.tree.selected_entry() {
            Some(row) if row.kind == EntryKind::Folder => row.path.clone(),
            Some(row) => row.path.parent().unwrap_or_default(),
            None => VaultPath::root(),
        };
        let parent = match &self.focus_folder {
            Some(folder) if !selected.starts_with(folder) => folder.clone(),
            _ => selected,
        };
        self.open(Prompt::Create {
            parent,
            kind,
            input: InputLine::default(),
        });
    }

    /// El nombre con el cursor antes de la extensión, para cambiar solo el
    /// nombre.
    /// En la vista de tags, renombra el tag elegido; en la de carpetas, la
    /// entrada elegida.
    pub fn start_rename(&mut self) {
        if self.view == PanelView::Tags {
            if let Some(tag) = self.selected_tag() {
                let input = InputLine::new(&tag);
                self.open(Prompt::RenameTag { tag, input });
            }
            return;
        }
        let Some(path) = self.tree.selected().cloned() else {
            return;
        };
        let name = path.name();
        let stem = name.rfind('.').filter(|&dot| dot > 0).unwrap_or(name.len());
        let input = InputLine::with_cursor(name, stem);
        self.open(Prompt::Rename { path, input });
    }

    /// En la vista de tags, fusiona el tag elegido con otro; en la de
    /// carpetas, mueve la entrada elegida.
    pub fn start_move(&mut self) {
        if self.view == PanelView::Tags {
            if let Some(tag) = self.selected_tag() {
                self.open(Prompt::MergeTag {
                    tag,
                    input: InputLine::new(""),
                });
            }
            return;
        }
        let Some(path) = self.tree.selected().cloned() else {
            return;
        };
        let input = InputLine::new(path.as_str());
        self.open(Prompt::Move { path, input });
    }

    /// En la vista de tags, pregunta `field` del tag elegido.
    pub fn start_tag_field(&mut self, field: TagField) {
        if let Some(tag) = self.selected_tag() {
            let input = InputLine::new(&field.current(self.tags.tags(), &tag));
            self.open(Prompt::TagField { tag, field, input });
        }
    }

    /// Otro focus, o el mismo con otro índice: las dos vistas lo siguen.
    pub fn set_focus(&mut self, scope: Scope, in_scope: Option<Arc<InScope>>) {
        self.focus_folder.clone_from(&scope.folder);
        self.tree.set_scope(in_scope);
        self.tags.set_scope(scope);
        self.refresh_shown();
    }

    /// El focus de la fila elegida: una carpeta o un tag. Un archivo no es
    /// un alcance, ni una fila de los backlinks.
    #[must_use]
    pub fn selected_focus(&self) -> Option<Focus> {
        match self.view {
            PanelView::Folders => self
                .tree
                .selected_entry()
                .filter(|row| row.is_folder())
                .map(|row| Focus::folder(row.path.clone())),
            PanelView::Tags => self.selected_tag().as_deref().map(Focus::tag),
            PanelView::Backlinks => None,
        }
    }

    /// La forma establecida del tag de la fila elegida en la vista de tags.
    #[must_use]
    pub fn selected_tag(&self) -> Option<String> {
        self.tags
            .selected_entry()
            .and_then(|row| row.tag_name())
            .map(str::to_owned)
    }

    /// `unsaved`: si algún buffer de lo que se borraría tiene cambios.
    pub fn start_delete(&mut self, path: VaultPath, unsaved: bool) {
        self.open(Prompt::ConfirmDelete { path, unsaved });
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

    /// Las preguntas son de archivos: en la vista de tags no se abren.
    /// Solo en la vista de la que es la pregunta.
    fn open(&mut self, prompt: Prompt) {
        if self.view != prompt.view() {
            return;
        }
        self.error = None;
        self.prompt = Some(prompt);
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

    /// Cierra la pregunta y su error; en el filtro, además lo quita.
    pub fn cancel(&mut self) {
        self.error = None;
        if let Some(Prompt::Filter(_)) = self.prompt.take() {
            self.tree.set_filter("");
        }
    }

    /// `Enter`: cierra la pregunta y devuelve lo que hay que hacer. Con un
    /// nombre inválido la pregunta sigue abierta y el error queda en
    /// [`Panel::error`].
    pub fn confirm(&mut self) -> Option<Request> {
        let prompt = self.prompt.take()?;
        match request(&prompt) {
            Ok(request) => {
                self.error = None;
                request
            }
            Err(error) => {
                self.error = Some(error.to_string());
                self.prompt = Some(prompt);
                None
            }
        }
    }
}

/// Por qué no se acepta lo escrito: la pregunta sigue abierta con esto.
#[derive(Debug, thiserror::Error)]
enum PromptError {
    #[error(transparent)]
    Path(#[from] InvalidPath),
    #[error("`{0}` no es un nombre de tag válido")]
    TagName(String),
    #[error(transparent)]
    Field(#[from] FieldError),
}

/// `Ok(None)` si no hay nada que hacer: un nombre vacío o el mismo.
fn request(prompt: &Prompt) -> Result<Option<Request>, PromptError> {
    Ok(match prompt {
        Prompt::Filter(_) => None,
        Prompt::Create {
            parent,
            kind,
            input,
        } => new_entry(parent, *kind, input.text().trim())?,
        Prompt::Rename { path, input } => {
            let name = input.text().trim();
            let parent = path.parent().unwrap_or_default();
            renamed(
                path,
                (!name.is_empty()).then(|| parent.join(name)).transpose()?,
            )
        }
        Prompt::Move { path, input } => {
            let target = input.text().trim().trim_matches('/');
            renamed(
                path,
                (!target.is_empty())
                    .then(|| VaultPath::new(target))
                    .transpose()?,
            )
        }
        Prompt::ConfirmDelete { path, .. } => Some(Request::Trash(path.clone())),
        Prompt::RenameTag { tag, input } => other_tag(tag, input)?.map(|to| {
            Request::ChangeTags(TagRequest::Rename {
                from: tag.clone(),
                to,
            })
        }),
        Prompt::TagField { tag, field, input } => Some(Request::ChangeTags(TagRequest::Page {
            tag: tag.clone(),
            field: *field,
            change: field.change(input.text())?,
        })),
        Prompt::MergeTag { tag, input } => other_tag(tag, input)?.map(|into| {
            Request::ChangeTags(TagRequest::Merge {
                from: tag.clone(),
                into,
            })
        }),
    })
}

/// Lo escrito como otro tag, sin `#`; vacío o igual a `tag`, ninguno.
fn other_tag(tag: &str, input: &InputLine) -> Result<Option<String>, PromptError> {
    let text = input.text().trim();
    if text.is_empty() {
        return Ok(None);
    }
    let name = tag_name(text).ok_or_else(|| PromptError::TagName(text.to_owned()))?;
    Ok((name != tag).then(|| name.to_owned()))
}

/// Como en los exploradores de nvim, un nombre que termina en `/` es una
/// carpeta y uno con `/` en medio crea las carpetas que falten. A una nota
/// sin `.md` se le agrega.
fn new_entry(
    parent: &VaultPath,
    kind: NewEntry,
    name: &str,
) -> Result<Option<Request>, InvalidPath> {
    let (name, kind) = match name.strip_suffix('/') {
        Some(folder) => (folder, NewEntry::Folder),
        None => (name, kind),
    };
    if name.is_empty() {
        return Ok(None);
    }
    let relative = match kind {
        NewEntry::Folder => name.to_owned(),
        NewEntry::Note if note_name::is_note(name) => name.to_owned(),
        NewEntry::Note => note_name::with_extension(name),
    };
    let path = if parent.is_root() {
        VaultPath::new(&relative)?
    } else {
        VaultPath::new(&format!("{}/{relative}", parent.as_str()))?
    };
    Ok(Some(match kind {
        NewEntry::Note => Request::CreateNote(path),
        NewEntry::Folder => Request::CreateFolder(path),
    }))
}

fn renamed(from: &VaultPath, to: Option<VaultPath>) -> Option<Request> {
    let to = to.filter(|to| to != from)?;
    Some(Request::Rename {
        from: from.clone(),
        to,
    })
}

#[cfg(test)]
mod focus_tests;
#[cfg(test)]
mod tag_field_tests;
#[cfg(test)]
mod tests;
