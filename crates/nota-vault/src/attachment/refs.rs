//! Qué adjuntos de la carpeta usa alguna nota. Un link sin `!` también
//! cuenta: borrar un PDF enlazado rompería el link igual que un embed.

use std::collections::{HashMap, HashSet};

use nota_syntax::{Resolution, resolve_from, scan};

use crate::index::{EntryKind, VaultIndex};
use crate::path::VaultPath;

/// Los adjuntos de una carpeta y cuáles están referidos. Se le pasan las
/// notas una por una ([`AttachmentRefs::add_note`]) o, cuando haya índice de
/// links, los destinos ya resueltos ([`AttachmentRefs::mark_used`]).
#[derive(Debug)]
pub struct AttachmentRefs<'i> {
    /// Los archivos del vault, contra los que se resuelve cada link.
    paths: HashSet<&'i str>,
    /// Los adjuntos de la carpeta por nombre en minúsculas: solo un link
    /// que termina en uno de estos nombres puede usarlos y vale resolverlo.
    by_name: HashMap<String, Vec<&'i VaultPath>>,
    used: HashSet<&'i str>,
}

impl<'i> AttachmentRefs<'i> {
    /// Los adjuntos de `folder` y sus subcarpetas, ninguno usado todavía.
    #[must_use]
    pub fn new(index: &'i VaultIndex, folder: &VaultPath) -> Self {
        let mut by_name: HashMap<String, Vec<&'i VaultPath>> = HashMap::new();
        for entry in index.files() {
            if entry.kind == EntryKind::Attachment && entry.path.starts_with(folder) {
                by_name
                    .entry(entry.path.name().to_lowercase())
                    .or_default()
                    .push(entry.path);
            }
        }
        Self {
            paths: index.files().map(|entry| entry.path.as_str()).collect(),
            by_name,
            used: HashSet::new(),
        }
    }

    /// Marca lo que enlaza o embebe la nota `source` con texto `text`. Un
    /// link ambiguo marca a todos sus candidatos: ante la duda, se conserva.
    pub fn add_note(&mut self, source: &VaultPath, text: &str) {
        for link in scan(text).links {
            let target = link.target_text(text);
            let name = target.rsplit('/').next().unwrap_or_default().to_lowercase();
            if !self.by_name.contains_key(&name) {
                continue;
            }
            match resolve_from(source.as_str(), &target, self.paths.iter().copied()) {
                Resolution::Resolved(path) => {
                    self.used.insert(path);
                }
                Resolution::Ambiguous(paths) => self.used.extend(paths),
                Resolution::Unresolved => {}
            }
        }
    }

    /// Marca un destino ya resuelto, como lo dará el índice de links.
    pub fn mark_used(&mut self, path: &VaultPath) {
        if let Some(known) = self.paths.get(path.as_str()) {
            self.used.insert(known);
        }
    }

    /// Los adjuntos de la carpeta que nada usa, por ruta.
    #[must_use]
    pub fn orphans(&self) -> Vec<&'i VaultPath> {
        let mut orphans: Vec<_> = self
            .by_name
            .values()
            .flatten()
            .copied()
            .filter(|path| !self.used.contains(path.as_str()))
            .collect();
        orphans.sort();
        orphans
    }
}
