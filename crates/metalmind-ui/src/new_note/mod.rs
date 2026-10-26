//! Notas nuevas desde una plantilla y la nota del día (06 §11), sin GPUI:
//! qué se pidió con `:template` o desde un tag, la pregunta por el nombre y
//! la creación en el vault. Las plantillas se leen al empezar cada pedido.

mod question;

use std::fmt::Write as _;

use jiff::Zoned;
use metalmind_config::Daily;
use metalmind_template::{
    DailyError, Draft, NewNote, NotePath, Pick, Templates, daily_note, draft, tag_of_template,
    tags_asking_for,
};
use metalmind_vault::tags::TagGraph;
use metalmind_vault::{FileOpError, InvalidPath, Vault, VaultIndex, VaultPath};

use crate::picker::PickerItem;
use crate::workspace::LinePosition;

pub use self::question::{NameProblem, NameQuestion};

/// Lo de `config.yaml` que usan las notas nuevas.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NoteSettings {
    pub templates_folder: String,
    pub daily: Daily,
}

/// `:template [plantilla [nombre]]`: lo que no se dijo se elige después.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteRequest {
    pub template: Option<String>,
    /// Puede llevar espacios: es todo lo que sigue a la plantilla.
    pub name: Option<String>,
}

impl NoteRequest {
    #[must_use]
    pub fn parse(argument: Option<&str>) -> Self {
        let argument = argument.unwrap_or_default().trim();
        let (template, name) = argument
            .split_once(char::is_whitespace)
            .map_or((argument, ""), |(template, name)| (template, name.trim()));
        let given = |text: &str| (!text.is_empty()).then(|| text.to_owned());
        Self {
            template: given(template),
            name: given(name),
        }
    }
}

/// De dónde sale el texto de la nota nueva.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    /// Una plantilla elegida; si la pide la página de un solo tag, la nota
    /// nace con ese tag.
    Template(String),
    /// Una nota nueva con este tag, con la plantilla que pida su página.
    Tag(String),
}

impl Origin {
    #[must_use]
    pub fn draft(
        &self,
        title: &str,
        templates: &Templates,
        graph: &TagGraph,
        now: &Zoned,
    ) -> Draft {
        let (tags, pick) = match self {
            Self::Template(name) => (
                tag_of_template(graph, name)
                    .map(str::to_owned)
                    .into_iter()
                    .collect(),
                Pick::Named(name),
            ),
            Self::Tag(tag) => (vec![tag.clone()], Pick::ByTags),
        };
        let note = NewNote {
            title,
            tags: &tags,
            pick,
            now,
        };
        draft(&note, templates, graph)
    }

    /// Lo que la pregunta del nombre dice de dónde sale la nota.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Template(name) => format!("plantilla {name}"),
            Self::Tag(tag) => format!("#{tag}"),
        }
    }
}

/// Las plantillas del vault, de la carpeta `folder` de la config.
///
/// # Errors
/// Si `folder` no es una ruta del vault.
pub fn load_templates(
    vault: &Vault,
    index: &VaultIndex,
    folder: &str,
) -> Result<Templates, InvalidPath> {
    let folder = VaultPath::new(folder.trim_end_matches('/'))?;
    let loaded = Templates::load(vault.root(), index, &folder);
    for error in loaded.errors {
        log::warn!("no se leyó una plantilla: {error}");
    }
    Ok(loaded.value)
}

/// Una plantilla en el picker, con los tags que la piden a la derecha.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TemplateItem {
    pub name: String,
    tags: Option<String>,
}

impl AsRef<str> for TemplateItem {
    fn as_ref(&self) -> &str {
        &self.name
    }
}

impl PickerItem for TemplateItem {
    fn detail(&self) -> Option<&str> {
        self.tags.as_deref()
    }
}

/// Lo que lista el picker de plantillas, en orden por nombre.
#[must_use]
pub fn template_items(templates: &Templates, graph: &TagGraph) -> Vec<TemplateItem> {
    templates
        .names()
        .map(|name| {
            let mut tags = String::new();
            for tag in tags_asking_for(graph, name) {
                let space = if tags.is_empty() { "" } else { " " };
                let _ = write!(tags, "{space}#{tag}");
            }
            TemplateItem {
                name: name.to_owned(),
                tags: (!tags.is_empty()).then_some(tags),
            }
        })
        .collect()
}

/// Una nota recién creada o, si ya existía, la de siempre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Landing {
    pub path: VaultPath,
    /// Dónde empezar a escribir; `None` si la nota ya existía.
    pub cursor: Option<LinePosition>,
    /// Lo que salió distinto de lo pedido al llenar la plantilla.
    pub problems: Vec<String>,
}

impl Landing {
    fn created(path: VaultPath, draft: &Draft) -> Self {
        Self {
            path,
            cursor: Some(line_position(&draft.text, draft.cursor)),
            problems: draft.problems.iter().map(ToString::to_string).collect(),
        }
    }
}

/// Crea `note` con `draft`. Nunca pisa: si ya existe, falla.
///
/// # Errors
/// Si ya existe o no se pudo escribir.
pub fn create(vault: &Vault, note: &NotePath, draft: &Draft) -> Result<Landing, FileOpError> {
    vault.create_note(&note.path, &draft.text)?;
    Ok(Landing::created(note.path.clone(), draft))
}

#[derive(Debug, thiserror::Error)]
pub enum DailyOpenError {
    #[error("la nota del día está apagada: se prende con `daily.enabled: true` en config.yaml")]
    Disabled,
    #[error(transparent)]
    Settings(#[from] DailyError),
    #[error(transparent)]
    Create(#[from] FileOpError),
}

/// La nota del día de `now`: la crea con su plantilla si no existe.
///
/// # Errors
/// Si las notas del día están apagadas o mal configuradas, o no se pudo
/// crear.
pub fn open_daily(
    vault: &Vault,
    daily: &Daily,
    templates: &Templates,
    now: &Zoned,
) -> Result<Landing, DailyOpenError> {
    let note = daily_note(daily, now, templates)?.ok_or(DailyOpenError::Disabled)?;
    match vault.create_note(&note.path, &note.draft.text) {
        Ok(()) => Ok(Landing::created(note.path, &note.draft)),
        Err(FileOpError::AlreadyExists(_)) => Ok(Landing {
            path: note.path,
            cursor: None,
            problems: Vec::new(),
        }),
        Err(error) => Err(error.into()),
    }
}

/// La línea y la columna en bytes del byte `at` de `text`.
fn line_position(text: &str, at: usize) -> LinePosition {
    let before = text.get(..at).unwrap_or(text);
    let start = before.rfind('\n').map_or(0, |newline| newline + 1);
    LinePosition {
        line: before.matches('\n').count(),
        column: before.len() - start,
    }
}

#[cfg(test)]
mod tests;
