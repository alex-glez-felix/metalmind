//! Lo que el panel le pregunta al usuario (un nombre, el filtro o confirmar
//! un borrado). Confirmar devuelve un [`Request`] que ejecuta quien tiene el
//! vault; así las reglas de los nombres se prueban sin disco.

use metalmind_syntax::note_name;
use metalmind_vault::tags::tag_name;
use metalmind_vault::{EntryKind, InvalidPath, VaultPath};

use super::{Panel, PanelView};
use crate::picker::InputLine;
use crate::tag_change::{FieldError, TagField, TagRequest};

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
    pub(super) fn view(&self) -> PanelView {
        match self {
            Self::RenameTag { .. } | Self::MergeTag { .. } | Self::TagField { .. } => {
                PanelView::Tags
            }
            _ => PanelView::Folders,
        }
    }

    /// La entrada sobre la que actúa: la que se renombra, mueve o borra, o
    /// la carpeta donde se crea. El filtro no tiene.
    pub(super) fn target_mut(&mut self) -> Option<&mut VaultPath> {
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

    /// El campo que se está editando, si la pregunta tiene uno.
    pub(super) fn input_mut(&mut self) -> Option<&mut InputLine> {
        match self {
            Self::Filter(input)
            | Self::Create { input, .. }
            | Self::Rename { input, .. }
            | Self::Move { input, .. }
            | Self::RenameTag { input, .. }
            | Self::MergeTag { input, .. }
            | Self::TagField { input, .. } => Some(input),
            Self::ConfirmDelete { .. } => None,
        }
    }

    /// `Ok(None)` si no hay nada que hacer: un nombre vacío o el mismo.
    pub(super) fn request(&self) -> Result<Option<Request>, PromptError> {
        Ok(match self {
            Self::Filter(_) => None,
            Self::Create {
                parent,
                kind,
                input,
            } => new_entry(parent, *kind, input.text().trim())?,
            Self::Rename { path, input } => {
                let name = input.text().trim();
                let parent = path.parent().unwrap_or_default();
                renamed(
                    path,
                    (!name.is_empty()).then(|| parent.join(name)).transpose()?,
                )
            }
            Self::Move { path, input } => {
                let target = input.text().trim().trim_matches('/');
                renamed(
                    path,
                    (!target.is_empty())
                        .then(|| VaultPath::new(target))
                        .transpose()?,
                )
            }
            Self::ConfirmDelete { path, .. } => Some(Request::Trash(path.clone())),
            Self::RenameTag { tag, input } => other_tag(tag, input)?.map(|to| {
                Request::ChangeTags(TagRequest::Rename {
                    from: tag.clone(),
                    to,
                })
            }),
            Self::TagField { tag, field, input } => Some(Request::ChangeTags(TagRequest::Page {
                tag: tag.clone(),
                field: *field,
                change: field.change(input.text())?,
            })),
            Self::MergeTag { tag, input } => other_tag(tag, input)?.map(|into| {
                Request::ChangeTags(TagRequest::Merge {
                    from: tag.clone(),
                    into,
                })
            }),
        })
    }
}

impl Panel {
    /// El campo que se está editando, si hay uno.
    pub fn input_mut(&mut self) -> Option<&mut InputLine> {
        self.prompt.as_mut()?.input_mut()
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

    /// `unsaved`: si algún buffer de lo que se borraría tiene cambios.
    pub fn start_delete(&mut self, path: VaultPath, unsaved: bool) {
        self.open(Prompt::ConfirmDelete { path, unsaved });
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
        match prompt.request() {
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

/// Por qué no se acepta lo escrito: la pregunta sigue abierta con esto.
#[derive(Debug, thiserror::Error)]
pub(super) enum PromptError {
    #[error(transparent)]
    Path(#[from] InvalidPath),
    #[error("`{0}` no es un nombre de tag válido")]
    TagName(String),
    #[error(transparent)]
    Field(#[from] FieldError),
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
