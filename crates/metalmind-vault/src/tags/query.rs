//! Consultas de tags (05 §4): `#rust #nota -#archivado`, y `-#` para las
//! notas sin tags, el buzón (06 §9).

use std::collections::HashSet;
use std::ops::Range;

use crate::path::VaultPath;
use crate::tags::Tags;
use crate::tags::key::TagKey;

/// Espacio es "y", `-` es "no". Un tag incluye a sus alias y a sus
/// descendientes por `parents`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TagQuery {
    pub include: Vec<TagKey>,
    pub exclude: Vec<TagKey>,
    /// Solo las notas sin ningún tag.
    pub untagged: bool,
}

/// Una consulta escrita, separada en lo que es de tags y lo demás.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedQuery {
    pub query: TagQuery,
    /// Las palabras que no son de tags, en orden: el texto para el fuzzy.
    pub words: Vec<String>,
    /// Lo que empieza con `#` o `-#` pero no es un tag válido (`#2024/10`).
    pub invalid: Vec<String>,
}

/// Qué es cada palabra de una consulta escrita.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryWord {
    /// Texto que no es de tags: va al fuzzy.
    Text,
    Include(TagKey),
    Exclude(TagKey),
    /// `-#`: las notas sin tags.
    Untagged,
    /// Empieza con `#` o `-#` pero no es un tag válido (`#2024/10`).
    Invalid,
}

impl QueryWord {
    fn of(word: &str) -> Self {
        let (negated, tag) = match word.strip_prefix('-') {
            Some(rest) if rest.starts_with('#') => (true, rest),
            _ => (false, word),
        };
        let Some(name) = tag.strip_prefix('#') else {
            return Self::Text;
        };
        if negated && name.is_empty() {
            return Self::Untagged;
        }
        if !metalmind_syntax::is_valid_tag_name(name) {
            return Self::Invalid;
        }
        let key = TagKey::new(name);
        if negated {
            Self::Exclude(key)
        } else {
            Self::Include(key)
        }
    }
}

impl TagQuery {
    /// Cada palabra de `text`, con sus bytes, y qué es.
    pub fn words(text: &str) -> impl Iterator<Item = (Range<usize>, QueryWord)> + '_ {
        let mut from = 0;
        std::iter::from_fn(move || {
            let start = from + text[from..].find(|c: char| !c.is_whitespace())?;
            let end = text[start..]
                .find(char::is_whitespace)
                .map_or(text.len(), |len| start + len);
            from = end;
            Some((start..end, QueryWord::of(&text[start..end])))
        })
    }

    #[must_use]
    pub fn parse(text: &str) -> ParsedQuery {
        let mut parsed = ParsedQuery::default();
        for (range, word) in Self::words(text) {
            let (list, key) = match word {
                QueryWord::Text => {
                    parsed.words.push(text[range].to_owned());
                    continue;
                }
                QueryWord::Invalid => {
                    parsed.invalid.push(text[range].to_owned());
                    continue;
                }
                QueryWord::Untagged => {
                    parsed.query.untagged = true;
                    continue;
                }
                QueryWord::Include(key) => (&mut parsed.query.include, key),
                QueryWord::Exclude(key) => (&mut parsed.query.exclude, key),
            };
            if !list.contains(&key) {
                list.push(key);
            }
        }
        parsed
    }

    /// Sin nada que filtre: da todas las notas.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.include.is_empty() && self.exclude.is_empty() && !self.untagged
    }

    /// Las notas que cumplen la consulta, en orden de ruta. Las páginas de
    /// tag nunca están.
    #[must_use]
    pub fn notes<'t>(&self, tags: &'t Tags) -> Vec<&'t VaultPath> {
        let mut found: HashSet<&VaultPath> = match self.include.split_first() {
            Some((first, rest)) => {
                let mut found: HashSet<&VaultPath> = tags.notes_with(first).collect();
                for key in rest {
                    let with: HashSet<&VaultPath> = tags.notes_with(key).collect();
                    found.retain(|note| with.contains(note));
                }
                found
            }
            None => tags.index.notes().map(|(path, _)| path).collect(),
        };
        if self.untagged {
            found.retain(|path| {
                tags.index
                    .note(path)
                    .is_none_or(|note| note.occurrences.is_empty())
            });
        }
        for key in &self.exclude {
            for note in tags.notes_with(key) {
                found.remove(note);
            }
        }
        let mut found: Vec<&VaultPath> = found.into_iter().collect();
        found.sort_unstable();
        found
    }
}
