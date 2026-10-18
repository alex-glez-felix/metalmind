//! El alcance del modo focus (05 §3): una carpeta, una consulta de tags o
//! las dos. Reduce lo que ofrecen el finder, la búsqueda, el panel y los
//! backlinks, no lo que se puede abrir.

use std::collections::HashSet;

use crate::path::VaultPath;
use crate::tags::Tags;
use crate::tags::query::TagQuery;

/// `:focus proyectos/ #rust -#archivado`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scope {
    pub folder: Option<VaultPath>,
    pub tags: TagQuery,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScopeError {
    #[error("`{0}` no es un tag válido")]
    InvalidTag(String),
    #[error("`{0}` no es una carpeta del vault")]
    InvalidFolder(String),
    #[error("un alcance tiene una sola carpeta: `{0}` y `{1}`")]
    TwoFolders(String, String),
}

impl Scope {
    /// Lo que no empieza con `#` ni `-#` es la carpeta; su `/` final es
    /// opcional.
    ///
    /// # Errors
    /// Si un tag no es válido, la carpeta no es una ruta del vault o hay más
    /// de una carpeta.
    pub fn parse(text: &str) -> Result<Self, ScopeError> {
        let parsed = TagQuery::parse(text);
        if let Some(invalid) = parsed.invalid.into_iter().next() {
            return Err(ScopeError::InvalidTag(invalid));
        }
        let folder = match parsed.words.as_slice() {
            [] => None,
            [folder] => {
                let trimmed = folder.trim_end_matches('/');
                let path = VaultPath::new(trimmed)
                    .ok()
                    .filter(|path| !path.is_root())
                    .ok_or_else(|| ScopeError::InvalidFolder(folder.clone()))?;
                Some(path)
            }
            [first, second, ..] => {
                return Err(ScopeError::TwoFolders(first.clone(), second.clone()));
            }
        };
        Ok(Self {
            folder,
            tags: parsed.query,
        })
    }

    /// Sin focus: todo el vault.
    #[must_use]
    pub fn is_everything(&self) -> bool {
        self.folder.is_none() && self.tags.is_empty()
    }

    /// Resuelve las consultas de tags una vez, para preguntar por muchas
    /// rutas.
    #[must_use]
    pub fn filter<'t>(&self, tags: &'t Tags) -> ScopeFilter<'t> {
        let notes =
            (!self.tags.is_empty()).then(|| self.tags.notes(tags).into_iter().cloned().collect());
        ScopeFilter {
            tags,
            everything: self.is_everything(),
            folder: self.folder.clone(),
            notes,
        }
    }

    /// Los tags con los que nace una nota nueva en este focus (05 §3), en su
    /// forma establecida: los incluidos, no los excluidos.
    #[must_use]
    pub fn new_note_tags(&self, tags: &Tags) -> Vec<String> {
        self.tags
            .include
            .iter()
            .map(|key| {
                let key = tags.graph().resolve(key);
                tags.display_name(key).unwrap_or(key.as_str()).to_owned()
            })
            .collect()
    }
}

/// Un [`Scope`] listo para preguntar qué entra.
#[derive(Debug, Clone)]
pub struct ScopeFilter<'t> {
    tags: &'t Tags,
    everything: bool,
    folder: Option<VaultPath>,
    /// Las notas que cumplen la consulta de tags, si hay consulta.
    notes: Option<HashSet<VaultPath>>,
}

impl ScopeFilter<'_> {
    /// Si el alcance es todo el vault.
    #[must_use]
    pub fn is_everything(&self) -> bool {
        self.everything
    }

    /// Si `path` (una nota o un adjunto) está en el alcance. Las páginas de
    /// tag no son notas normales (05 §7): solo entran con `:focus tags/`.
    #[must_use]
    pub fn contains(&self, path: &VaultPath) -> bool {
        if self.tags.is_page(path) {
            return self.folder.as_ref() == Some(self.tags.folder());
        }
        let in_folder = self
            .folder
            .as_ref()
            .is_none_or(|folder| path.starts_with(folder) && path != folder);
        in_folder && self.notes.as_ref().is_none_or(|notes| notes.contains(path))
    }
}
