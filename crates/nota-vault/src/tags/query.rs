//! Consultas de tags (05 §4): `#rust #nota -#archivado`, y `-#` para las
//! notas sin tags, el buzón (06 §9).

use std::collections::HashSet;

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

impl TagQuery {
    #[must_use]
    pub fn parse(text: &str) -> ParsedQuery {
        let mut parsed = ParsedQuery::default();
        for word in text.split_whitespace() {
            let (negated, tag) = match word.strip_prefix('-') {
                Some(rest) if rest.starts_with('#') => (true, rest),
                _ => (false, word),
            };
            if negated && tag == "#" {
                parsed.query.untagged = true;
                continue;
            }
            let Some(name) = tag.strip_prefix('#') else {
                parsed.words.push(word.to_owned());
                continue;
            };
            let Some(key) = nota_syntax::is_valid_tag_name(name).then(|| TagKey::new(name)) else {
                parsed.invalid.push(word.to_owned());
                continue;
            };
            let list = if negated {
                &mut parsed.query.exclude
            } else {
                &mut parsed.query.include
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
                let mut found = tags.notes_with(first);
                for key in rest {
                    let with = tags.notes_with(key);
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
