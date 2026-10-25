use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use metalmind_syntax::Property;

use crate::path::VaultPath;
use crate::tags::key::TagKey;
use crate::tags::note::NoteTags;

/// Lo que las notas escriben: qué notas llevan cada tag, y con qué
/// ocurrencias, para renombrar sin reescanear, y el frontmatter entero de
/// cada una. Las páginas de tag guardan sus ocurrencias (se reescriben al
/// renombrar) pero no cuentan en `by_tag`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TagIndex {
    by_tag: HashMap<TagKey, TagEntry>,
    by_note: HashMap<VaultPath, IndexedNote>,
}

/// Un tag tal como se escribe en las notas, antes de resolver alias o padres.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TagEntry {
    /// Las que llevan este tag exacto.
    pub notes: BTreeSet<VaultPath>,
    /// Cada forma escrita y cuántas veces aparece.
    pub spellings: BTreeMap<String, u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IndexedNote {
    pub tags: NoteTags,
    pub page: bool,
    /// Compartido: cada lote del watcher clona el índice entero.
    pub frontmatter: Option<Arc<[Property]>>,
}

impl TagIndex {
    #[must_use]
    pub fn tag(&self, key: &TagKey) -> Option<&TagEntry> {
        self.by_tag.get(key)
    }

    /// Los tags que aparecen en alguna nota, sin orden.
    pub fn keys(&self) -> impl Iterator<Item = &TagKey> {
        self.by_tag.keys()
    }

    /// Las ocurrencias de una nota o página, en orden de aparición.
    #[must_use]
    pub fn note(&self, path: &VaultPath) -> Option<&NoteTags> {
        self.by_note.get(path).map(|note| &note.tags)
    }

    /// Cada clave del frontmatter de una nota o página con su valor, en
    /// orden; vacío si no tiene, no se pudo leer o no es un mapa.
    #[must_use]
    pub fn frontmatter(&self, path: &VaultPath) -> &[Property] {
        self.by_note
            .get(path)
            .and_then(|note| note.frontmatter.as_deref())
            .unwrap_or_default()
    }

    /// Las notas indexadas que no son páginas de tag, sin orden.
    pub fn notes(&self) -> impl Iterator<Item = (&VaultPath, &NoteTags)> {
        self.by_note
            .iter()
            .filter(|(_, note)| !note.page)
            .map(|(path, note)| (path, &note.tags))
    }

    /// Todas las notas y páginas indexadas, sin orden.
    pub(crate) fn all(&self) -> impl Iterator<Item = (&VaultPath, &IndexedNote)> {
        self.by_note.iter()
    }

    /// Reemplaza lo que había de `path` y devuelve las claves que tocó.
    pub(crate) fn insert(&mut self, path: VaultPath, note: IndexedNote) -> BTreeSet<TagKey> {
        let mut touched = self.remove(&path);
        if !note.page {
            for occurrence in &note.tags.occurrences {
                let entry = self.by_tag.entry(occurrence.key.clone()).or_default();
                entry.notes.insert(path.clone());
                *entry
                    .spellings
                    .entry(occurrence.spelling.clone())
                    .or_default() += 1;
                touched.insert(occurrence.key.clone());
            }
        }
        self.by_note.insert(path, note);
        touched
    }

    /// Quita `path` y devuelve las claves que tocó.
    pub(crate) fn remove(&mut self, path: &VaultPath) -> BTreeSet<TagKey> {
        self.take(path)
            .map(|(_, touched)| touched)
            .unwrap_or_default()
    }

    /// Quita `path` y devuelve lo que tenía y las claves que tocó.
    pub(crate) fn take(&mut self, path: &VaultPath) -> Option<(IndexedNote, BTreeSet<TagKey>)> {
        let note = self.by_note.remove(path)?;
        let mut touched = BTreeSet::new();
        if !note.page {
            for occurrence in &note.tags.occurrences {
                self.forget(path, &occurrence.key, &occurrence.spelling);
                touched.insert(occurrence.key.clone());
            }
        }
        Some((note, touched))
    }

    fn forget(&mut self, path: &VaultPath, key: &TagKey, spelling: &str) {
        let Some(entry) = self.by_tag.get_mut(key) else {
            return;
        };
        entry.notes.remove(path);
        if let Some(count) = entry.spellings.get_mut(spelling) {
            *count -= 1;
            if *count == 0 {
                entry.spellings.remove(spelling);
            }
        }
        if entry.notes.is_empty() {
            self.by_tag.remove(key);
        }
    }
}
