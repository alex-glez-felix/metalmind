//! Tests de los tags sobre vaults en memoria: cada caso es una lista de
//! archivos con su contenido.

mod graph;
mod query;

use std::collections::{BTreeMap, BTreeSet};

use crate::index::{EntryKind, VaultIndex};
use crate::path::VaultPath;
use crate::tags::{DEFAULT_TAGS_FOLDER, TagKey, Tags};

pub(super) fn path(path: &str) -> VaultPath {
    VaultPath::new(path).unwrap()
}

pub(super) fn key(name: &str) -> TagKey {
    TagKey::new(name)
}

/// Un vault con estos archivos (y las carpetas que les hagan falta).
pub(super) struct Fixture {
    pub files: BTreeMap<VaultPath, String>,
}

impl Fixture {
    pub fn new(files: &[(&str, &str)]) -> Self {
        Self {
            files: files
                .iter()
                .map(|(file, text)| (path(file), (*text).to_owned()))
                .collect(),
        }
    }

    pub fn index(&self) -> VaultIndex {
        let mut entries = BTreeSet::new();
        for file in self.files.keys() {
            for folder in file.ancestors().filter(|folder| !folder.is_root()) {
                entries.insert((folder, EntryKind::Folder));
            }
            entries.insert((file.clone(), EntryKind::of_file(file)));
        }
        VaultIndex::from_entries(entries.into_iter().collect())
    }

    pub fn tags(&self) -> Tags {
        Tags::build(
            VaultPath::fixed(DEFAULT_TAGS_FOLDER),
            &self.index(),
            |file: &VaultPath| self.files.get(file).cloned(),
        )
    }
}

pub(super) fn tags(files: &[(&str, &str)]) -> Tags {
    Fixture::new(files).tags()
}
