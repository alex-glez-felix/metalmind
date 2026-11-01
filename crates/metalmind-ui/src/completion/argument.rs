//! La lista del autocompletado de la palabra que se escribe en el
//! argumento de un comando de `:` (#156), sin GPUI. En `:focus`, tras `#` o
//! `-#` van los tags, los mismos que en `#` dentro de una nota, y si no las
//! carpetas del vault; en `:e`, sus archivos (#45). Aceptar escribe la
//! palabra entera, con sus escapes.

use std::collections::BTreeSet;
use std::sync::Arc;

use metalmind_search::FuzzyMatcher;
use metalmind_syntax::TagPlace;
use metalmind_vault::VaultPath;
use metalmind_vault::tags::TagCompletion;
use metalmind_vim::{ArgumentWord, escape_argument};

use super::{PathCandidates, PathCompletions, Selection, TagCompletions};
use crate::focus::folder_argument;

/// Lo que puede ofrecer `:focus`, ya preparado para filtrar.
pub struct FocusSources<'a> {
    pub tags: Arc<[TagCompletion]>,
    pub folders: Arc<PathCandidates>,
    /// Para nombres de tags.
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
    Folders(PathCompletions),
    /// Los archivos de `:e`.
    Files(PathCompletions),
}

/// Lo que pide una palabra de `:focus`.
enum FocusWord<'w> {
    Tag { typed: &'w str, negated: bool },
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
        sources: FocusSources<'_>,
        previous: Option<Self>,
    ) -> Option<Self> {
        if word.range.is_empty() {
            return None;
        }
        let previous = Self::same_word(word, previous);
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
                following(list, sources.folders, typed, sources.paths).map(ArgumentList::Folders)
            }
            (FocusWord::Folder(typed), _) => {
                PathCompletions::open(sources.folders, typed, sources.paths)
                    .map(ArgumentList::Folders)
            }
        }?;
        Some(Self {
            word: word.clone(),
            list,
        })
    }

    /// La lista de la ruta `word` de `:e` entre los archivos `files`. Si
    /// sigue a la de `previous` con más escrito, solo filtra entre lo que
    /// esta mostraba; con lo mismo escrito, como [`ArgumentCompletions::focus`].
    /// Una ruta que no puede ser del vault (absoluta, con `..`) no tiene
    /// lista: lo que se escribe es relativo a la raíz del vault.
    #[must_use]
    pub fn edit(
        word: &ArgumentWord,
        files: Arc<PathCandidates>,
        matcher: &mut FuzzyMatcher,
        previous: Option<Self>,
    ) -> Option<Self> {
        let typed = word.text.as_str();
        if typed.is_empty() || VaultPath::new(typed.trim_end_matches('/')).is_err() {
            return None;
        }
        let list = match previous.map(|previous| previous.list) {
            Some(ArgumentList::Files(list)) if list.typed() == typed => {
                following(list, files, typed, matcher)
            }
            Some(ArgumentList::Files(list)) if list.is_among(&files) => {
                list.retyped(typed, matcher)
            }
            _ => PathCompletions::open(files, typed, matcher),
        }?;
        Some(Self {
            word: word.clone(),
            list: ArgumentList::Files(list),
        })
    }

    /// La lista de `previous` si es de la misma palabra que `word`.
    fn same_word(word: &ArgumentWord, previous: Option<Self>) -> Option<ArgumentList> {
        previous
            .filter(|previous| previous.word.text == word.text)
            .filter(|previous| previous.word.literal_start == word.literal_start)
            .map(|previous| previous.list)
    }

    #[must_use]
    pub fn list(&self) -> &ArgumentList {
        &self.list
    }

    #[must_use]
    pub fn len(&self) -> usize {
        match &self.list {
            ArgumentList::Tags { list, .. } => list.rows().len(),
            ArgumentList::Folders(list) | ArgumentList::Files(list) => list.rows().len(),
        }
    }

    #[must_use]
    pub fn selection(&self) -> &Selection {
        match &self.list {
            ArgumentList::Tags { list, .. } => list.selection(),
            ArgumentList::Folders(list) | ArgumentList::Files(list) => list.selection(),
        }
    }

    pub fn selection_mut(&mut self) -> &mut Selection {
        match &mut self.list {
            ArgumentList::Tags { list, .. } => list.selection_mut(),
            ArgumentList::Folders(list) | ArgumentList::Files(list) => list.selection_mut(),
        }
    }

    /// La palabra que escribe aceptar en lugar de la escrita: `#Rust`,
    /// `-#Rust`, `Mis\ notas/` o `Mis\ notas/rust.md`.
    #[must_use]
    pub fn accept(&self) -> String {
        match &self.list {
            ArgumentList::Tags { list, negated } => {
                let sign = if *negated { "-#" } else { "#" };
                format!("{sign}{}", list.accept())
            }
            ArgumentList::Folders(list) => folder_argument(list.accept()),
            ArgumentList::Files(list) => escape_argument(list.accept().as_str()),
        }
    }
}

/// `list` entre `candidates`, con `typed` escrito: la misma si nada cambió.
fn following(
    list: PathCompletions,
    candidates: Arc<PathCandidates>,
    typed: &str,
    matcher: &mut FuzzyMatcher,
) -> Option<PathCompletions> {
    if list.is_for(&candidates, typed) {
        Some(list)
    } else {
        list.with_candidates(candidates, matcher)
    }
}

#[cfg(test)]
#[path = "argument_tests.rs"]
mod tests;
