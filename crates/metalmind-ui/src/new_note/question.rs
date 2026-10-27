//! La pregunta por el nombre de una nota nueva, como un picker de una sola
//! fila: la ruta que va a crear, o por qué todavía no puede.

use std::ops::Range;
use std::sync::Arc;

use gpui_kit::{Context, SharedString};
use jiff::Zoned;
use metalmind_template::{Draft, NameError, NotePath, Templates};
use metalmind_vault::tags::TagGraph;
use metalmind_vault::{VaultIndex, VaultPath};

use super::Origin;
use crate::focus::FocusedNote;
use crate::picker::{
    Chosen, InputLine, LineEdit, Open, Overflow, PickerDelegate, PickerRow, PickerView,
};

pub struct NameQuestion {
    line: InputLine,
    origin: Origin,
    templates: Templates,
    /// Para avisar mientras se escribe de un nombre tomado; crear igual
    /// falla si aparece después.
    index: Arc<VaultIndex>,
    /// Por qué no se pudo crear con lo escrito; se olvida al editarlo.
    failed: Option<String>,
    /// Dónde y con qué tags nace en el focus.
    focused: FocusedNote,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum NameProblem {
    #[error(transparent)]
    Name(#[from] NameError),
    #[error("{0} ya existe")]
    Exists(VaultPath),
    #[error("{0}")]
    Failed(String),
}

impl NameQuestion {
    #[must_use]
    pub fn new(origin: Origin, templates: Templates, index: Arc<VaultIndex>) -> Self {
        Self {
            line: InputLine::default(),
            origin,
            templates,
            index,
            failed: None,
            focused: FocusedNote::default(),
        }
    }

    /// En focus de carpeta, lo escrito va dentro de ella; en focus de tags,
    /// la nota nace con ellos (05 §3).
    #[must_use]
    pub fn in_focus(mut self, focused: FocusedNote) -> Self {
        self.focused = focused;
        self
    }

    #[must_use]
    pub fn origin(&self) -> &Origin {
        &self.origin
    }

    /// La nota que crearía lo escrito, con su texto.
    ///
    /// # Errors
    /// Los de [`NameQuestion::answer`].
    pub fn draft(&self, graph: &TagGraph, now: &Zoned) -> Result<(NotePath, Draft), NameProblem> {
        let note = self.answer()?;
        let draft = self
            .origin
            .draft(&note.title, &self.focused, &self.templates, graph, now);
        Ok((note, draft))
    }

    /// La nota que crearía lo escrito.
    ///
    /// # Errors
    /// Si falta el nombre, no es una ruta del vault, ya existe o falló el
    /// último intento con él.
    pub fn answer(&self) -> Result<NotePath, NameProblem> {
        if let Some(failed) = &self.failed {
            return Err(NameProblem::Failed(failed.clone()));
        }
        let name = self.line.text().trim();
        let note = match &self.focused.folder {
            Some(folder) if !name.is_empty() => NotePath::parse(&format!("{folder}/{name}"))?,
            _ => NotePath::parse(name)?,
        };
        if self.index.contains(&note.path) {
            return Err(NameProblem::Exists(note.path));
        }
        Ok(note)
    }

    /// Crear con lo escrito falló: la pregunta sigue abierta con la causa.
    pub fn fail(&mut self, reason: String) {
        self.failed = Some(reason);
    }

    pub fn apply(&mut self, edit: LineEdit<'_>) {
        if self.line.apply(edit) {
            self.failed = None;
        }
    }
}

impl PickerDelegate for NameQuestion {
    fn line(&self) -> &InputLine {
        &self.line
    }

    fn warnings(&self) -> Vec<Range<usize>> {
        match self.answer() {
            Err(NameProblem::Name(NameError::Empty)) | Ok(_) => Vec::new(),
            Err(_) => std::iter::once(0..self.line.text().len()).collect(),
        }
    }

    fn len(&self) -> usize {
        usize::from(self.answer().is_ok())
    }

    fn selected(&self) -> usize {
        0
    }

    fn select(&mut self, _: usize) {}

    fn select_next(&mut self) {}

    fn select_previous(&mut self) {}

    fn edit(&mut self, edit: LineEdit<'_>, _: &mut Context<PickerView<Self>>) {
        self.apply(edit);
    }

    fn choose(&mut self, open: Open, _: &mut Context<PickerView<Self>>) -> Option<Chosen> {
        self.answer().ok().map(|_| Chosen { item: 0, open })
    }

    fn row(&mut self, _: usize) -> PickerRow {
        let path = self
            .answer()
            .map(|note| note.path.as_str().to_owned())
            .unwrap_or_default();
        PickerRow {
            text: path,
            highlights: Vec::new(),
            overflow: Overflow::Path,
            detail: Some(self.origin.label()),
        }
    }

    fn empty(&self) -> SharedString {
        match self.answer() {
            Ok(_) => SharedString::default(),
            Err(NameProblem::Name(NameError::Empty)) => {
                SharedString::new_static("Escribe el nombre; enter crea la nota y la abre")
            }
            Err(problem) => problem.to_string().into(),
        }
    }
}
