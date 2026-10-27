//! El modo focus en la UI (05 §3), sin GPUI: el alcance como se escribió y
//! qué archivos y carpetas del vault quedan dentro de él.

use std::collections::HashSet;
use std::sync::Arc;

use metalmind_vault::tags::{Scope, ScopeError, TagKey, TagQuery, Tags};
use metalmind_vault::{VaultIndex, VaultPath};

/// Un alcance puesto: `:focus proyectos/ #rust`, o un nodo del panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Focus {
    scope: Scope,
    /// Lo que muestran la barra de estado, el panel y el finder.
    text: String,
}

impl Focus {
    /// El argumento de `:focus`; `None` si está vacío.
    ///
    /// # Errors
    /// Los de [`Scope::parse`].
    pub fn parse(text: &str) -> Result<Option<Self>, ScopeError> {
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if text.is_empty() {
            return Ok(None);
        }
        let scope = Scope::parse(&text)?;
        Ok(Some(Self { scope, text }))
    }

    /// `F` sobre una carpeta del panel.
    #[must_use]
    pub fn folder(path: VaultPath) -> Self {
        Self {
            text: format!("{path}/"),
            scope: Scope {
                folder: Some(path),
                tags: TagQuery::default(),
            },
        }
    }

    /// `F` sobre un tag del panel, por su forma establecida.
    #[must_use]
    pub fn tag(name: &str) -> Self {
        Self {
            text: format!("#{name}"),
            scope: Scope {
                folder: None,
                tags: TagQuery {
                    include: vec![TagKey::new(name)],
                    ..TagQuery::default()
                },
            },
        }
    }

    #[must_use]
    pub fn scope(&self) -> &Scope {
        &self.scope
    }

    /// `focus: #rust -#archivado`
    #[must_use]
    pub fn label(&self) -> String {
        format!("focus: {}", self.text)
    }

    /// Si es el mismo alcance que `other`, aunque se haya escrito distinto
    /// (`#Rust` y `#rust`).
    #[must_use]
    pub fn same_scope(&self, other: &Self) -> bool {
        self.scope == other.scope
    }

    /// Dónde y con qué tags nace una nota nueva en este focus, para que no
    /// desaparezca de él al crearla (05 §3).
    #[must_use]
    pub fn new_note(&self, tags: &Tags) -> FocusedNote {
        FocusedNote {
            folder: self.scope.folder.clone(),
            tags: self.scope.new_note_tags(tags),
        }
    }

    /// Qué hay dentro del alcance en `index` con estos `tags`.
    #[must_use]
    pub fn resolve(&self, index: &VaultIndex, tags: &Tags) -> InScope {
        let filter = self.scope.filter(tags);
        let files: HashSet<VaultPath> = index
            .files()
            .filter(|entry| filter.contains(entry.path))
            .map(|entry| entry.path.clone())
            .collect();
        let mut folders = HashSet::new();
        for file in &files {
            for folder in file.ancestors().filter(|folder| !folder.is_root()) {
                if !folders.insert(folder) {
                    break;
                }
            }
        }
        InScope {
            files: Arc::new(files),
            folders,
        }
    }
}

/// Una nota nueva en focus: dentro de su carpeta y con sus tags incluidos,
/// en su forma establecida. Sin focus, en la raíz y sin tags.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FocusedNote {
    pub folder: Option<VaultPath>,
    pub tags: Vec<String>,
}

/// Los archivos del vault dentro de un focus y las carpetas que los
/// contienen: las demás carpetas se esconden.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InScope {
    files: Arc<HashSet<VaultPath>>,
    folders: HashSet<VaultPath>,
}

impl InScope {
    /// Si el panel muestra `path`: un archivo del alcance o una carpeta con
    /// alguno dentro.
    #[must_use]
    pub fn shows(&self, path: &VaultPath) -> bool {
        self.files.contains(path) || self.folders.contains(path)
    }

    #[must_use]
    pub fn contains_file(&self, path: &VaultPath) -> bool {
        self.files.contains(path)
    }

    /// Los archivos, para la búsqueda de texto.
    #[must_use]
    pub fn files(&self) -> Arc<HashSet<VaultPath>> {
        Arc::clone(&self.files)
    }
}

#[cfg(test)]
mod tests;
