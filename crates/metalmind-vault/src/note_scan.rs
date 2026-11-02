use std::path::Path;

use metalmind_syntax::Property;

use crate::index::{EntryKind, VaultIndex};
use crate::links::note::links_of;
use crate::links::{IndexedLink, LinkIndex};
use crate::note_cache::{CacheRefresh, Stamp, open_notes};
use crate::path::VaultPath;
use crate::tags::{NoteTags, Tags, tags_of};
use crate::worker::read_note;

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

/// Los índices de links y tags de `files`, leyendo una vez las notas que la
/// caché no tenga al día.
pub(crate) fn index_notes(
    root: &Path,
    files: &VaultIndex,
    tags_folder: VaultPath,
    cache: Option<&Path>,
) -> (LinkIndex, Tags, Option<CacheRefresh>) {
    let notes = files
        .files()
        .filter(|entry| entry.kind == EntryKind::Note)
        .map(|entry| entry.path.clone())
        .collect();
    let read = |path: &VaultPath| read_note(root, path);
    let opened = open_notes(
        cache,
        notes,
        &|path: &VaultPath| Stamp::of_file(&path.to_fs(root)),
        &read,
    );
    let (links, tags) = opened
        .scans
        .into_iter()
        .map(|(path, scan)| {
            (
                (path.clone(), scan.links),
                (path, scan.tags, scan.properties),
            )
        })
        .unzip();
    let links = LinkIndex::from_scans(
        files.files().map(|entry| (entry.path.clone(), entry.kind)),
        links,
    );
    let tags = Tags::from_scans(tags_folder, tags, read);
    (links, tags, opened.refresh)
}
