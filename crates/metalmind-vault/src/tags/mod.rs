//! Los tags del vault (05): el índice de lo que escriben las notas, el grafo
//! de las páginas de tag, las consultas y los planes de renombrar y fusionar.

mod complete;
mod cycles;
mod graph;
mod index;
mod key;
mod layout;
mod metadata;
mod note;
mod page;
mod query;
mod refactor;
mod rename;
mod scope;
mod tally;
#[cfg(test)]
mod tests;
mod tree;
mod update;

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::sync::Arc;
use std::thread;

pub use complete::TagCompletion;
pub use graph::{TagGraph, TagProblem};
pub use index::{TagEntry, TagIndex};
pub use key::{TagKey, tag_name};
pub use layout::{PageEntry, PageLayout};
pub use metadata::{PageChange, yaml_scalar};
pub use note::{FrontmatterSpot, NoteTags, Snippet, TagOccurrence, TagSource};
pub use page::{InvalidValue, PageField, TagColor, TagIcon, TagPage, parse_template};
pub use query::{ParsedQuery, QueryWord, TagQuery};
pub use refactor::{PlanError, PlanNotice, TagPlan};
pub use scope::{Scope, ScopeError, ScopeFilter};
pub use tree::{TagNode, TagTree};

use crate::index::{EntryKind, VaultIndex};
use crate::path::VaultPath;
use index::IndexedNote;
use note::{PageFile, page_name, scan_note};

/// La carpeta de las páginas de tag si el vault no dice otra.
pub const DEFAULT_TAGS_FOLDER: &str = "tags";

/// Índice y grafo de los tags de un vault, siempre al día entre sí. Es un
/// valor inmutable para quien lo lee, como [`VaultIndex`]; clonarlo para
/// publicar un lote comparte todo lo que el lote no tocó.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tags {
    folder: VaultPath,
    index: TagIndex,
    /// Los archivos de la carpeta de tags, en orden de ruta. Son pocos y
    /// cambiar uno ya rehace el grafo: se copian enteros solo entonces.
    pages: Arc<BTreeMap<VaultPath, PageFile>>,
    /// Sale solo de `pages` y se rehace entero cuando cambian.
    graph: Arc<TagGraph>,
}

/// Lee una nota del vault; `None` si no se pudo.
pub trait ReadNote: Fn(&VaultPath) -> Option<String> + Sync {}

impl<F: Fn(&VaultPath) -> Option<String> + Sync> ReadNote for F {}

impl Tags {
    /// Escanea todas las notas de `vault`, en paralelo.
    #[must_use]
    pub fn build(folder: VaultPath, vault: &VaultIndex, read: impl ReadNote) -> Self {
        let notes: Vec<&VaultPath> = vault
            .files()
            .filter(|entry| entry.kind == EntryKind::Note)
            .map(|entry| entry.path)
            .collect();
        let mut tags = Self::empty(folder);
        let scanned = scan_all(&tags.folder, &notes, &read);
        for (path, note, page) in scanned {
            tags.store(path, note, page);
        }
        tags.graph = tags.rebuild_graph();
        tags
    }

    fn empty(folder: VaultPath) -> Self {
        Self {
            folder,
            index: TagIndex::default(),
            pages: Arc::default(),
            graph: Arc::default(),
        }
    }

    /// La carpeta de las páginas de tag.
    #[must_use]
    pub fn folder(&self) -> &VaultPath {
        &self.folder
    }

    #[must_use]
    pub fn index(&self) -> &TagIndex {
        &self.index
    }

    #[must_use]
    pub fn graph(&self) -> &TagGraph {
        &self.graph
    }

    /// Si el vault conoce `key`: lo escribe una nota, tiene página, es un
    /// alias que aplica o lo nombra algún `parents`.
    #[must_use]
    pub fn exists(&self, key: &TagKey) -> bool {
        self.index.tag(key).is_some()
            || self.graph.page(key).is_some()
            || self.graph.alias_of(key).is_some()
            || !self.graph.children(key).is_empty()
    }

    /// Las notas que trae `#key`: el tag, sus alias y sus descendientes con
    /// los suyos; una nota sale una vez por cada uno que lleve. Las páginas de
    /// tag no cuentan.
    pub(crate) fn notes_with(&self, key: &TagKey) -> impl Iterator<Item = &VaultPath> {
        let entries: Vec<&TagEntry> = self
            .graph
            .expansion(key)
            .iter()
            .filter_map(|key| self.index.tag(key))
            .collect();
        entries.into_iter().flat_map(|entry| &entry.notes)
    }

    /// Cuántas notas distintas trae `#key` (05 §2, conteo del panel).
    #[must_use]
    pub fn count(&self, key: &TagKey) -> usize {
        self.notes_with(key).collect::<HashSet<_>>().len()
    }

    /// Si `key` solo existe porque lo nombra un `parents`: ni nota ni página.
    #[must_use]
    pub fn is_only_parent(&self, key: &TagKey) -> bool {
        self.index.tag(key).is_none()
            && self.graph.page(key).is_none()
            && !self.graph.children(key).is_empty()
    }

    /// La forma establecida del tag (05 §6): la más escrita en las notas,
    /// contando el nombre de su página como una más; en un empate, la menor
    /// en orden de bytes. Un tag que solo nombra un `parents` se muestra como
    /// lo escribe ese `parents`.
    #[must_use]
    pub fn display_name(&self, key: &TagKey) -> Option<&str> {
        let mut counts: BTreeMap<&str, u32> = self
            .index
            .tag(key)
            .into_iter()
            .flat_map(|entry| &entry.spellings)
            .map(|(spelling, count)| (spelling.as_str(), *count))
            .collect();
        if let Some(page) = self.graph.page(key) {
            *counts.entry(&page.name).or_default() += 1;
        }
        let most_used = counts
            .into_iter()
            .max_by_key(|&(spelling, count)| (count, Reverse(spelling)))
            .map(|(spelling, _)| spelling);
        most_used.or_else(|| {
            self.graph
                .pages()
                .flat_map(|page| &page.parents)
                .find(|parent| &parent.key == key)
                .map(|parent| parent.spelling.as_str())
        })
    }

    /// Los tags que el panel muestra arriba (05 §3): los que no tienen
    /// padres y, de un ciclo sin padres fuera de él, todos sus tags, para que
    /// no desaparezcan. Los alias no son nodos. En orden de clave.
    #[must_use]
    pub fn roots(&self) -> Vec<TagKey> {
        self.node_keys()
            .filter(|key| self.graph.parents(key).is_empty() || self.in_closed_cycle(key))
            .cloned()
            .collect()
    }

    /// `key` está en un ciclo y ningún ancestro suyo es raíz.
    fn in_closed_cycle(&self, key: &TagKey) -> bool {
        let mut seen = BTreeSet::new();
        let mut pending = VecDeque::from([key]);
        let mut on_cycle = false;
        while let Some(next) = pending.pop_front() {
            for parent in self.graph.parents(next) {
                if self.graph.parents(parent).is_empty() {
                    return false;
                }
                on_cycle |= parent == key;
                if seen.insert(parent) {
                    pending.push_back(parent);
                }
            }
        }
        on_cycle
    }

    /// Los tags como si los buffers con cambios sin guardar (ruta y texto)
    /// ya estuvieran guardados. Un plan armado sobre esto trae, para esos
    /// archivos, ediciones sobre el texto del buffer, que la UI aplica al
    /// buffer como una transacción en vez de escribir el disco (05 §5).
    #[must_use]
    pub fn with_unsaved<'b>(
        &self,
        buffers: impl IntoIterator<Item = (&'b VaultPath, &'b str)>,
    ) -> Self {
        let mut tags = self.clone();
        let mut pages_changed = false;
        for (path, text) in buffers {
            let (note, page) = scan_text(&tags.folder, path, text);
            pages_changed |= tags.store(path.clone(), note, page).1;
        }
        if pages_changed {
            tags.graph = tags.rebuild_graph();
        }
        tags
    }

    /// Si `path` es una página de tag: no cuenta como nota normal (05 §7).
    #[must_use]
    pub fn is_page(&self, path: &VaultPath) -> bool {
        matches!(self.pages.get(path), Some(PageFile::Page(_)))
    }

    /// Guarda lo escaneado de una nota y si es página de tag. Devuelve las
    /// claves que tocó y si cambió alguna página, que obliga a rehacer el grafo.
    fn store(
        &mut self,
        path: VaultPath,
        note: IndexedNote,
        page: Option<PageFile>,
    ) -> (BTreeSet<TagKey>, bool) {
        let page_changed = self.pages.get(&path) != page.as_ref();
        match page {
            Some(page) if page_changed => {
                Arc::make_mut(&mut self.pages).insert(path.clone(), page);
            }
            Some(_) => {}
            None => {
                self.drop_page(&path);
            }
        }
        (self.index.insert(path, Arc::new(note)), page_changed)
    }

    /// Quita `path` de las páginas sin copiarlas si no estaba; dice si estaba.
    fn drop_page(&mut self, path: &VaultPath) -> bool {
        self.pages.contains_key(path) && Arc::make_mut(&mut self.pages).remove(path).is_some()
    }

    fn rebuild_graph(&self) -> Arc<TagGraph> {
        let pages = self.pages.values().filter_map(|file| match file {
            PageFile::Page(page) => Some(page.as_ref()),
            PageFile::InvalidName => None,
        });
        let invalid = self
            .pages
            .iter()
            .filter(|(_, file)| matches!(file, PageFile::InvalidName))
            .map(|(path, _)| path);
        Arc::new(TagGraph::build(pages, invalid))
    }
}

/// Escanea una nota y dice si es página de tag. Si no se pudo leer, cuenta
/// como una nota sin tags.
fn scan_one(
    folder: &VaultPath,
    path: &VaultPath,
    read: &impl ReadNote,
) -> (IndexedNote, Option<PageFile>) {
    scan_text(folder, path, &read(path).unwrap_or_default())
}

fn scan_text(folder: &VaultPath, path: &VaultPath, text: &str) -> (IndexedNote, Option<PageFile>) {
    let (tags, frontmatter) = scan_note(text);
    let page = page_name(folder, path).map(|name| match name {
        Ok(name) => PageFile::Page(Arc::new(TagPage::new(
            path.clone(),
            name,
            text,
            frontmatter.as_ref(),
        ))),
        Err(()) => PageFile::InvalidName,
    });
    let note = IndexedNote {
        tags,
        page: matches!(page, Some(PageFile::Page(_))),
        frontmatter: frontmatter
            .map(|frontmatter| frontmatter.properties)
            .unwrap_or_default(),
    };
    (note, page)
}

type Scanned = (VaultPath, IndexedNote, Option<PageFile>);

fn scan_all(folder: &VaultPath, notes: &[&VaultPath], read: &impl ReadNote) -> Vec<Scanned> {
    let workers = thread::available_parallelism().map_or(1, usize::from);
    let chunk = notes.len().div_ceil(workers).max(1);
    thread::scope(|scope| {
        let handles: Vec<_> = notes
            .chunks(chunk)
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .iter()
                        .map(|path| {
                            let (note, page) = scan_one(folder, path, read);
                            ((*path).clone(), note, page)
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| match handle.join() {
                Ok(scanned) => scanned,
                Err(panic) => std::panic::resume_unwind(panic),
            })
            .collect()
    })
}
