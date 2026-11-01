//! La lista del autocompletado de la palabra que se escribe en el
//! argumento de un comando de `:` (#156), sin GPUI. En `:focus`, tras `#` o
//! `-#` van los tags, los mismos que en `#` dentro de una nota, tras `@` las
//! vistas guardadas, y si no las carpetas del vault. Aceptar escribe la
//! palabra entera, con sus escapes.

use std::collections::BTreeSet;
use std::sync::Arc;

use metalmind_config::SavedView;
use metalmind_search::FuzzyMatcher;
use metalmind_syntax::TagPlace;
use metalmind_vault::tags::TagCompletion;
use metalmind_vim::{ArgumentWord, escape_argument};

use super::{FolderCandidates, FolderCompletions, Selection, TagCompletions, ViewCompletions};
use crate::focus::folder_argument;

/// Lo que se puede ofrecer, ya preparado para filtrar.
pub struct ArgumentSources<'a> {
    pub tags: Arc<[TagCompletion]>,
    pub folders: Arc<FolderCandidates>,
    pub views: Arc<[SavedView]>,
    /// Para nombres de tags y de vistas.
    pub names: &'a mut FuzzyMatcher,
    /// Para rutas.
    pub paths: &'a mut FuzzyMatcher,
}

pub struct ArgumentCompletions {
    /// La palabra para la que se abrió, como se escribió.
    word: ArgumentWord,
    list: ArgumentList,
}

/// Qué se ofrece.
pub enum ArgumentList {
    /// `#ru` o, con `negated`, `-#ru`.
    Tags {
        list: TagCompletions,
        negated: bool,
    },
    Folders(FolderCompletions),
    /// `@gs`.
    Views(ViewCompletions),
}

/// Lo que pide una palabra de `:focus`.
enum FocusWord<'w> {
    Tag { typed: &'w str, negated: bool },
    View(&'w str),
    Folder(&'w str),
}

impl<'w> FocusWord<'w> {
    fn of(word: &'w ArgumentWord) -> Self {
        let text = word.text.as_str();
        if !word.literal_start {
            if let Some(typed) = text.strip_prefix("-#") {
                return Self::Tag {
                    typed,
                    negated: true,
                };
            }
            if let Some(typed) = text.strip_prefix('#') {
                return Self::Tag {
                    typed,
                    negated: false,
                };
            }
            if let Some(typed) = text.strip_prefix('@') {
                return Self::View(typed);
            }
        }
        Self::Folder(text)
    }
}

impl ArgumentCompletions {
    /// La lista de `word` en `:focus`, siguiendo a `previous` si es la de la
    /// misma palabra: si eligió una fila, sigue elegida. `None` sin nada
    /// escrito o si nada coincide.
    #[must_use]
    pub fn focus(
        word: &ArgumentWord,
        sources: ArgumentSources<'_>,
        previous: Option<Self>,
    ) -> Option<Self> {
        if word.range.is_empty() {
            return None;
        }
        let previous = previous
            .filter(|previous| previous.word.text == word.text)
            .filter(|previous| previous.word.literal_start == word.literal_start)
            .map(|previous| previous.list);
        let list = match (FocusWord::of(word), previous) {
            (FocusWord::Tag { typed, negated }, Some(ArgumentList::Tags { list, .. })) => {
                if list.is_for(&sources.tags, typed, TagPlace::Inline) {
                    Some(list)
                } else {
                    list.with_candidates(sources.tags, &BTreeSet::new(), sources.names)
                }
                .map(|list| ArgumentList::Tags { list, negated })
            }
            (FocusWord::Tag { typed, negated }, _) => TagCompletions::open(
                sources.tags,
                typed,
                TagPlace::Inline,
                &BTreeSet::new(),
                sources.names,
            )
            .map(|list| ArgumentList::Tags { list, negated }),
            (FocusWord::Folder(typed), Some(ArgumentList::Folders(list))) => {
                if list.is_for(&sources.folders, typed) {
                    Some(list)
                } else {
                    list.with_candidates(sources.folders, sources.paths)
                }
                .map(ArgumentList::Folders)
            }
            (FocusWord::Folder(typed), _) => {
                FolderCompletions::open(sources.folders, typed, sources.paths)
                    .map(ArgumentList::Folders)
            }
            (FocusWord::View(typed), Some(ArgumentList::Views(list))) => {
                if list.is_for(&sources.views, typed) {
                    Some(list)
                } else {
                    list.with_views(sources.views, sources.names)
                }
                .map(ArgumentList::Views)
            }
            (FocusWord::View(typed), _) => {
                ViewCompletions::open(sources.views, typed, sources.names).map(ArgumentList::Views)
            }
        }?;
        Some(Self {
            word: word.clone(),
            list,
        })
    }

    #[must_use]
    pub fn list(&self) -> &ArgumentList {
        &self.list
    }

    #[must_use]
    pub fn len(&self) -> usize {
        match &self.list {
            ArgumentList::Tags { list, .. } => list.rows().len(),
            ArgumentList::Folders(list) => list.rows().len(),
            ArgumentList::Views(list) => list.rows().len(),
        }
    }

    #[must_use]
    pub fn selection(&self) -> &Selection {
        match &self.list {
            ArgumentList::Tags { list, .. } => list.selection(),
            ArgumentList::Folders(list) => list.selection(),
            ArgumentList::Views(list) => list.selection(),
        }
    }

    pub fn selection_mut(&mut self) -> &mut Selection {
        match &mut self.list {
            ArgumentList::Tags { list, .. } => list.selection_mut(),
            ArgumentList::Folders(list) => list.selection_mut(),
            ArgumentList::Views(list) => list.selection_mut(),
        }
    }

    /// La palabra que escribe aceptar en lugar de la escrita: `#Rust`,
    /// `-#Rust`, `@Para\ hoy` o `Mis\ notas/`.
    #[must_use]
    pub fn accept(&self) -> String {
        match &self.list {
            ArgumentList::Tags { list, negated } => {
                let sign = if *negated { "-#" } else { "#" };
                format!("{sign}{}", list.accept())
            }
            ArgumentList::Folders(list) => folder_argument(list.accept()),
            ArgumentList::Views(list) => format!("@{}", escape_argument(&list.accept().name)),
        }
    }
}

#[cfg(test)]
#[path = "argument_tests.rs"]
mod tests;
