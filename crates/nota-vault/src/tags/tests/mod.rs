//! Tests de los tags sobre vaults en memoria: cada caso es una lista de
//! archivos con su contenido.

mod complexity;
mod graph;
mod metadata;
mod panel;
mod query;
mod rename;

use std::collections::{BTreeMap, BTreeSet};

use crate::changes::Rename;
use crate::index::{EntryKind, VaultIndex};
use crate::path::VaultPath;
use crate::tags::{DEFAULT_TAGS_FOLDER, TagKey, TagPlan, Tags};

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

/// Los archivos tras aplicar el plan, con la página ya renombrada.
pub(super) fn applied(fixture: &Fixture, plan: &TagPlan) -> BTreeMap<VaultPath, String> {
    let mut files = fixture.files.clone();
    for edit in &plan.changes.edits {
        let text = edit.apply(&fixture.files[&edit.path]).unwrap();
        files.insert(edit.path.clone(), text);
    }
    for file in &plan.changes.creates {
        assert!(files.insert(file.path.clone(), file.text.clone()).is_none());
    }
    for Rename { from, to } in &plan.changes.renames {
        let text = files.remove(from).unwrap();
        files.insert(to.clone(), text);
    }
    files
}

/// Solo lo que cambió, como `ruta: contenido`.
pub(super) fn changed(fixture: &Fixture, plan: &TagPlan) -> Vec<(String, String)> {
    applied(fixture, plan)
        .into_iter()
        .filter(|(file, text)| fixture.files.get(file) != Some(text))
        .map(|(file, text)| (file.to_string(), text))
        .collect()
}

pub(super) fn owned(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(file, text)| ((*file).to_owned(), (*text).to_owned()))
        .collect()
}
