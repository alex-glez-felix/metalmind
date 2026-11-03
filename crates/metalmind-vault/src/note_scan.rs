use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};

use metalmind_syntax::Property;

use crate::index::{EntryKind, VaultIndex};
use crate::links::note::links_of;
use crate::links::{IndexedLink, LinkIndex};
use crate::note_cache::{CacheRefresh, Stamp, open_notes};
use crate::path::VaultPath;
use crate::properties::PropertyIndex;
use crate::tags::{NoteTags, ReadNote, Tags, page_name, tags_of};

/// Lo que los índices sacan del texto de una nota, sin saber nada del resto
/// del vault: sus links sin resolver, sus tags y su frontmatter.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct NoteScan {
    pub links: Vec<IndexedLink>,
    pub tags: NoteTags,
    pub properties: Vec<Property>,
}

impl NoteScan {
    pub(crate) fn of(text: &str) -> Self {
        let scan = metalmind_syntax::scan(text);
        Self {
            links: links_of(text, &scan.links),
            tags: tags_of(text, &scan),
            properties: scan
                .frontmatter
                .map(|frontmatter| frontmatter.properties)
                .unwrap_or_default(),
        }
    }
}

/// Lo que sale de abrir las notas de un vault.
pub(crate) struct NoteIndexes {
    pub links: LinkIndex,
    pub tags: Tags,
    pub properties: PropertyIndex,
    pub refresh: Option<CacheRefresh>,
}

/// Los índices de links, tags y propiedades de `files`, leyendo una vez las notas que la
/// caché no tenga al día y las páginas de tag. `read` da el contenido de una
/// nota, o `None` si no se pudo leer.
pub(crate) fn index_notes(
    root: &Path,
    files: &VaultIndex,
    tags_folder: &VaultPath,
    cache: Option<&Path>,
    read: &impl ReadNote,
) -> NoteIndexes {
    let notes = files
        .files()
        .filter(|entry| entry.kind == EntryKind::Note)
        .map(|entry| entry.path.clone())
        .collect();
    let reads = PageReads {
        folder: tags_folder,
        read,
        pages: Mutex::default(),
    };
    let opened = open_notes(
        cache,
        notes,
        &|path: &VaultPath| Stamp::of_file(&path.to_fs(root)),
        &|path: &VaultPath| reads.note(path),
    );
    let mut links = Vec::with_capacity(opened.scans.len());
    let mut tags = Vec::with_capacity(opened.scans.len());
    let mut properties = Vec::with_capacity(opened.scans.len());
    for (path, scan) in opened.scans {
        links.push((path.clone(), scan.links));
        tags.push((path.clone(), scan.tags));
        properties.push((path, scan.properties));
    }
    NoteIndexes {
        links: LinkIndex::from_scans(
            files.files().map(|entry| (entry.path.clone(), entry.kind)),
            links,
        ),
        tags: Tags::from_scans(tags_folder.clone(), tags, |page: &VaultPath| {
            reads.page(page)
        }),
        properties: PropertyIndex::from_scans(properties),
        refresh: opened.refresh,
    }
}

/// Las lecturas de una apertura: lo que se lea de una página de tag se recuerda
/// porque [`Tags::from_scans`] necesita su texto. De las demás notas no se
/// guarda, para no tener el vault entero en memoria.
struct PageReads<'a, R> {
    folder: &'a VaultPath,
    read: &'a R,
    pages: Mutex<HashMap<VaultPath, Option<String>>>,
}

impl<R: ReadNote> PageReads<'_, R> {
    fn note(&self, path: &VaultPath) -> Option<String> {
        let text = (self.read)(path);
        if page_name(self.folder, path).is_some() {
            self.remembered().insert(path.clone(), text.clone());
        }
        text
    }

    /// El texto de `page`: el ya leído, o el del disco si la caché la tenía.
    fn page(&self, page: &VaultPath) -> Option<String> {
        let remembered = self.remembered().remove(page);
        remembered.unwrap_or_else(|| (self.read)(page))
    }

    fn remembered(&self) -> MutexGuard<'_, HashMap<VaultPath, Option<String>>> {
        self.pages.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs::{self, File};
    use std::time::{Duration, SystemTime};

    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    use super::*;
    use crate::scan::scan;
    use crate::tags::DEFAULT_TAGS_FOLDER;
    use crate::worker::read_note;

    /// Cuántas veces se leyó cada nota al abrir `root`.
    fn open_counting(root: &Path, cache: &Path) -> (BTreeMap<String, usize>, Tags) {
        let reads = Mutex::new(BTreeMap::new());
        let read = |path: &VaultPath| {
            let mut reads = reads.lock().unwrap();
            *reads.entry(path.to_string()).or_default() += 1;
            read_note(root, path)
        };
        let indexes = index_notes(
            root,
            &scan(root, None),
            &VaultPath::fixed(DEFAULT_TAGS_FOLDER),
            Some(cache),
            &read,
        );
        if let Some(refresh) = &indexes.refresh {
            refresh.write(&indexes.links, &indexes.tags, &indexes.properties);
        }
        (reads.into_inner().unwrap(), indexes.tags)
    }

    #[test]
    fn abrir_lee_cada_nota_una_sola_vez() {
        let (vault, data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
        let root = vault.path().canonicalize().unwrap();
        let cache = data.path().join("notas");
        fs::create_dir(root.join("tags")).unwrap();
        for (name, text) in [
            ("a.md", "[[b]] #rust"),
            ("b.md", "#gpui"),
            ("tags/rust.md", "---\ncolor: naranja\n---\n"),
            ("tags/mi tag.md", "#rust"),
        ] {
            fs::write(root.join(name), text).unwrap();
            // Un mtime reciente no es de fiar y la caché leería la nota.
            File::options()
                .write(true)
                .open(root.join(name))
                .unwrap()
                .set_modified(SystemTime::now() - Duration::from_secs(3_600))
                .unwrap();
        }
        let once = |names: &[&str]| -> BTreeMap<String, usize> {
            names.iter().map(|name| ((*name).to_owned(), 1)).collect()
        };

        let (reads, tags) = open_counting(&root, &cache);
        assert_eq!(
            reads,
            once(&["a.md", "b.md", "tags/mi tag.md", "tags/rust.md"]),
            "sin caché"
        );
        let (reads, cached) = open_counting(&root, &cache);
        assert_eq!(
            reads,
            once(&["tags/mi tag.md", "tags/rust.md"]),
            "con la caché al día solo faltan las páginas"
        );
        assert_eq!(cached, tags);
    }

    #[test]
    fn una_pagina_ilegible_tampoco_se_lee_dos_veces() {
        let vault = TempDir::new().unwrap();
        let root = vault.path().canonicalize().unwrap();
        fs::create_dir(root.join("tags")).unwrap();
        fs::write(root.join("tags/rust.md"), "").unwrap();
        let reads = Mutex::new(0);
        let read = |_: &VaultPath| {
            *reads.lock().unwrap() += 1;
            None
        };
        let indexes = index_notes(
            &root,
            &scan(&root, None),
            &VaultPath::fixed(DEFAULT_TAGS_FOLDER),
            None,
            &read,
        );
        assert_eq!(reads.into_inner().unwrap(), 1);
        assert!(
            indexes
                .tags
                .is_page(&VaultPath::new("tags/rust.md").unwrap())
        );
    }
}
