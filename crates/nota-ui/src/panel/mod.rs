//! El panel lateral sin GPUI: si se ve, en qué vista, el árbol de carpetas
//! y lo que se le está preguntando al usuario (un nombre, el filtro o
//! confirmar un borrado). Confirmar devuelve un [`Request`] que ejecuta
//! quien tiene el vault; así las reglas de los nombres se prueban sin disco.

mod input;

use nota_vault::{EntryKind, InvalidPath, VaultPath};

pub use self::input::LineInput;
use crate::file_tree::FileTree;

/// Las dos vistas del panel (05 §3). La de tags llega con el índice de
/// tags (M2.5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PanelView {
    #[default]
    Folders,
    Tags,
}

impl PanelView {
    #[must_use]
    pub fn other(self) -> Self {
        match self {
            Self::Folders => Self::Tags,
            Self::Tags => Self::Folders,
        }
    }
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
    Filter(LineInput),
    Create {
        parent: VaultPath,
        kind: NewEntry,
        input: LineInput,
    },
    Rename {
        path: VaultPath,
        input: LineInput,
    },
    /// Como renombrar, pero se escribe la ruta entera.
    Move {
        path: VaultPath,
        input: LineInput,
    },
    ConfirmDelete {
        path: VaultPath,
        /// Algún buffer de lo que se borra tiene cambios sin guardar.
        unsaved: bool,
    },
}

/// Una operación de archivo que el usuario confirmó.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    CreateNote(VaultPath),
    CreateFolder(VaultPath),
    Rename { from: VaultPath, to: VaultPath },
    Trash(VaultPath),
}

#[derive(Clone, Debug)]
pub struct Panel {
    pub visible: bool,
    pub view: PanelView,
    pub tree: FileTree,
    prompt: Option<Prompt>,
    /// El último error de una operación, hasta la próxima.
    error: Option<String>,
}

impl Panel {
    #[must_use]
    pub fn new(tree: FileTree, visible: bool, view: PanelView) -> Self {
        Self {
            visible,
            view,
            tree,
            prompt: None,
            error: None,
        }
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

    /// Muestra `view`, o esconde el panel si ya la mostraba. Devuelve si
    /// quedó visible.
    pub fn toggle(&mut self, view: PanelView) -> bool {
        self.visible = !(self.visible && self.view == view);
        self.view = view;
        self.visible
    }

    /// Un clic en la fila `at`, el `count`-ésimo seguido: selecciona, el
    /// primero abre o cierra una carpeta y el segundo sobre un archivo
    /// devuelve el archivo a abrir. El segundo clic sobre una carpeta no
    /// hace nada: si no, un doble clic la dejaría como estaba.
    pub fn click(&mut self, at: usize, count: usize) -> Option<VaultPath> {
        self.tree.select_row(at);
        let row = self.tree.selected_entry()?;
        let path = row.path.clone();
        match (row.kind, count) {
            (EntryKind::Folder, 1) => {
                self.tree.toggle(&path);
                None
            }
            (EntryKind::Folder, _) | (_, ..=1) => None,
            _ => Some(path),
        }
    }

    /// El campo que se está editando, si hay uno.
    pub fn input_mut(&mut self) -> Option<&mut LineInput> {
        match self.prompt.as_mut()? {
            Prompt::Filter(input)
            | Prompt::Create { input, .. }
            | Prompt::Rename { input, .. }
            | Prompt::Move { input, .. } => Some(input),
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
        self.open(Prompt::Filter(LineInput::new(self.tree.filter())));
    }

    /// Dentro de la carpeta seleccionada, o junto al archivo seleccionado.
    pub fn start_create(&mut self, kind: NewEntry) {
        let parent = match self.tree.selected_entry() {
            Some(row) if row.kind == EntryKind::Folder => row.path.clone(),
            Some(row) => row.path.parent().unwrap_or_default(),
            None => VaultPath::root(),
        };
        self.open(Prompt::Create {
            parent,
            kind,
            input: LineInput::default(),
        });
    }

    /// El nombre con el cursor antes de la extensión, para cambiar solo el
    /// nombre.
    pub fn start_rename(&mut self) {
        let Some(path) = self.tree.selected().cloned() else {
            return;
        };
        let name = path.name();
        let stem = name.rfind('.').filter(|&dot| dot > 0).unwrap_or(name.len());
        let input = LineInput::with_cursor(name, stem);
        self.open(Prompt::Rename { path, input });
    }

    pub fn start_move(&mut self) {
        let Some(path) = self.tree.selected().cloned() else {
            return;
        };
        let input = LineInput::new(path.as_str());
        self.open(Prompt::Move { path, input });
    }

    /// `unsaved`: si algún buffer de lo que se borraría tiene cambios.
    pub fn start_delete(&mut self, path: VaultPath, unsaved: bool) {
        self.open(Prompt::ConfirmDelete { path, unsaved });
    }

    fn open(&mut self, prompt: Prompt) {
        self.error = None;
        self.prompt = Some(prompt);
    }

    /// `Escape`: cierra la pregunta; en el filtro, además lo quita.
    pub fn cancel(&mut self) {
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

/// `Ok(None)` si no hay nada que hacer: un nombre vacío o el mismo.
fn request(prompt: &Prompt) -> Result<Option<Request>, InvalidPath> {
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
    })
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
        NewEntry::Note if has_md_extension(name) => name.to_owned(),
        NewEntry::Note => format!("{name}.md"),
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

fn has_md_extension(name: &str) -> bool {
    name.rsplit_once('.')
        .is_some_and(|(stem, ext)| !stem.is_empty() && ext.eq_ignore_ascii_case("md"))
}

fn renamed(from: &VaultPath, to: Option<VaultPath>) -> Option<Request> {
    let to = to.filter(|to| to != from)?;
    Some(Request::Rename {
        from: from.clone(),
        to,
    })
}

#[cfg(test)]
mod tests;
