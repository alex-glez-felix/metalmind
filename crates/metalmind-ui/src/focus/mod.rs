//! El modo focus en la UI (05 §3), sin GPUI: el alcance como se escribió y
//! qué archivos y carpetas del vault quedan dentro de él.

use std::collections::HashSet;
use std::sync::Arc;

use metalmind_config::SavedView;
use metalmind_vault::tags::{
    QueryWord, Scope, ScopeError, ScopeWord, TagGraph, TagKey, TagQuery, Tags,
};
use metalmind_vault::{VaultChange, VaultIndex, VaultPath};
use metalmind_vim::{ArgumentWord, argument_words, escape_argument};

/// Un alcance puesto: `:focus proyectos/ #rust`, o un nodo del panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Focus {
    scope: Scope,
    /// Lo que muestran la barra de estado, el panel y el finder, en el
    /// orden en que se escribió.
    words: Vec<Written>,
}

/// Una palabra del alcance como se muestra.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Written {
    /// La del alcance, que puede haberse movido.
    Folder,
    /// Tal cual se escribió: `#Rust` no se muestra como `#rust`.
    Tag(String),
    /// `@gs`: una vista guardada (06 §10), con la consulta que tiene.
    View { name: String, query: String },
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FocusError {
    #[error(transparent)]
    Scope(#[from] ScopeError),
    #[error("no hay ninguna vista «{0}»")]
    UnknownView(String),
    #[error("la vista «{name}» no es solo de tags: `{word}` no se puede enfocar")]
    NotTags { name: String, word: String },
}

impl Focus {
    /// El argumento de `:focus`; `None` si está vacío. Un espacio de una
    /// carpeta se escribe `\ ` o entre comillas (ver [`folder_argument`]),
    /// y `@nombre` es la vista de `views` con ese nombre, que suma sus tags.
    ///
    /// # Errors
    /// Los de [`Scope::from_words`], una vista que no está o una que no es
    /// solo de tags.
    pub fn parse(text: &str, views: &[SavedView]) -> Result<Option<Self>, FocusError> {
        let words = argument_words(text);
        if words.is_empty() {
            return Ok(None);
        }
        let mut scope_words = Vec::with_capacity(words.len());
        let mut written = Vec::with_capacity(words.len());
        for word in &words {
            if let Some(name) = view_name(word) {
                let view = crate::note_list::view_named(views, name)
                    .ok_or_else(|| FocusError::UnknownView(name.to_owned()))?;
                scope_words.extend(view_tags(view)?.into_iter().map(ScopeWord::Tag));
                written.push(Written::of_view(view));
                continue;
            }
            let scope_word = scope_word(word);
            written.push(match scope_word {
                ScopeWord::Tag(tag) => Written::Tag(tag.to_owned()),
                ScopeWord::Folder(_) => Written::Folder,
            });
            scope_words.push(scope_word);
        }
        let scope = Scope::from_words(scope_words)?;
        Ok(Some(Self {
            scope,
            words: written,
        }))
    }

    /// `F` sobre una vista del panel.
    ///
    /// # Errors
    /// Si su consulta no es solo de tags.
    pub fn view(view: &SavedView) -> Result<Self, FocusError> {
        let tags = view_tags(view)?.join(" ");
        Ok(Self {
            scope: Scope {
                folder: None,
                tags: TagQuery::parse(&tags).query,
            },
            words: vec![Written::of_view(view)],
        })
    }

    /// `F` sobre una carpeta del panel.
    #[must_use]
    pub fn folder(path: VaultPath) -> Self {
        Self {
            words: vec![Written::Folder],
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
            words: vec![Written::Tag(format!("#{name}"))],
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

    /// `focus: Mis\ notas/ #rust -#archivado`: se puede volver a escribir en
    /// `:focus` tal cual.
    #[must_use]
    pub fn label(&self) -> String {
        let words: Vec<String> = self
            .words
            .iter()
            .filter_map(|word| match word {
                Written::Tag(tag) => Some(tag.clone()),
                Written::Folder => self.scope.folder.as_ref().map(folder_argument),
                Written::View { name, .. } => Some(format!("@{}", escape_argument(name))),
            })
            .collect();
        format!("focus: {}", words.join(" "))
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
            excluded: self.scope.tags.exclude.clone(),
            untagged: self.scope.tags.untagged,
        }
    }

    /// Sigue a su carpeta si un lote del watcher la renombra o la mueve.
    /// Devuelve si cambió.
    pub fn follow(&mut self, changes: &[VaultChange]) -> bool {
        let Some(folder) = &self.scope.folder else {
            return false;
        };
        let mut moved = folder.clone();
        for change in changes {
            if let VaultChange::Renamed { from, to, .. } = change
                && let Some(rebased) = moved.rebase(from, to)
            {
                moved = rebased;
            }
        }
        if &moved == folder {
            return false;
        }
        self.scope.folder = Some(moved);
        true
    }

    /// Sigue a las vistas que nombra cuando cambian las del vault: a su
    /// nombre nuevo si `renamed` dice que una se renombró, y a su consulta
    /// de ahora. Una que ya no está, o que dejó de ser solo de tags, queda
    /// como los tags que tenía: el alcance no cambia sin que se pida.
    /// Devuelve si cambió.
    pub fn follow_views(&mut self, views: &[SavedView], renamed: Option<(&str, &str)>) -> bool {
        let mut words = Vec::with_capacity(self.words.len());
        for word in &self.words {
            let Written::View { name, query } = word else {
                words.push(word.clone());
                continue;
            };
            let name = match renamed {
                Some((from, to)) if from == name => to,
                _ => name,
            };
            match views
                .iter()
                .find(|view| view.name == name)
                .filter(|view| view_tags(view).is_ok())
            {
                Some(view) => words.push(Written::of_view(view)),
                None => words.extend(
                    TagQuery::words(query).map(|(range, _)| Written::Tag(query[range].to_owned())),
                ),
            }
        }
        if words == self.words {
            return false;
        }
        let tags: Vec<&str> = words
            .iter()
            .filter_map(|word| match word {
                Written::Tag(tag) => Some(tag.as_str()),
                Written::View { query, .. } => Some(query.as_str()),
                Written::Folder => None,
            })
            .collect();
        self.scope.tags = TagQuery::parse(&tags.join(" ")).query;
        self.words = words;
        true
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

/// Una carpeta como se escribe en `:focus` y como la muestra el
/// indicador: `Mis\ notas/`, y `\#ideas/` o `\@ideas/` si parece un tag o
/// una vista.
#[must_use]
pub fn folder_argument(folder: &VaultPath) -> String {
    let escaped = escape_argument(folder.as_str());
    let looks_like_folder = matches!(ScopeWord::of(folder.as_str()), ScopeWord::Folder(_))
        && !folder.as_str().starts_with('@');
    if looks_like_folder {
        format!("{escaped}/")
    } else {
        format!("\\{escaped}/")
    }
}

impl Written {
    fn of_view(view: &SavedView) -> Self {
        Self::View {
            name: view.name.clone(),
            query: view.query.clone(),
        }
    }
}

/// `@gs` nombra la vista `gs`; escapado (`\@gs`), es una carpeta.
fn view_name(word: &ArgumentWord) -> Option<&str> {
    word.text.strip_prefix('@').filter(|_| !word.literal_start)
}

/// Las palabras de la consulta de `view`, si todas son de tags.
fn view_tags(view: &SavedView) -> Result<Vec<&str>, FocusError> {
    TagQuery::words(&view.query)
        .map(|(range, word)| match word {
            QueryWord::Text | QueryWord::Invalid => Err(FocusError::NotTags {
                name: view.name.clone(),
                word: view.query[range].to_owned(),
            }),
            _ => Ok(&view.query[range]),
        })
        .collect()
}

/// Lo escapado o entre comillas al principio no es un tag: `\#ideas/`.
fn scope_word(word: &ArgumentWord) -> ScopeWord<'_> {
    if word.literal_start {
        ScopeWord::Folder(&word.text)
    } else {
        ScopeWord::of(&word.text)
    }
}

/// Una nota nueva en focus: dentro de su carpeta y con sus tags incluidos,
/// en su forma establecida. Sin focus, en la raíz y sin tags.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FocusedNote {
    pub folder: Option<VaultPath>,
    pub tags: Vec<String>,
    /// Lo que el focus deja fuera: una plantilla elegida no trae esos tags.
    excluded: Vec<TagKey>,
    /// `-#`: el focus son las notas sin tags.
    untagged: bool,
}

impl FocusedNote {
    /// Si una nota con `tag` quedaría fuera del focus: el tag, uno de sus
    /// ancestros o un alias de ellos está excluido.
    #[must_use]
    pub fn excludes(&self, graph: &TagGraph, tag: &str) -> bool {
        if self.untagged {
            return true;
        }
        let key = TagKey::new(tag);
        let excluded: Vec<&TagKey> = self.excluded.iter().map(|key| graph.resolve(key)).collect();
        graph
            .ancestors(graph.resolve(&key))
            .into_iter()
            .any(|ancestor| excluded.contains(&ancestor))
    }
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
