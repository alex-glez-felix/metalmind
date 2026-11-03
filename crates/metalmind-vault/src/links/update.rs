//! Armar el índice y mantenerlo al día con los cambios del vault. Cada
//! resolución es función de los archivos del vault y de la nota de origen,
//! así que aplicar cambios llega al mismo índice que armarlo de cero.

use std::collections::{BTreeSet, HashMap as StdHashMap, HashSet};
use std::sync::Arc;

use imbl::{HashMap, OrdSet};

use super::names::key_of;
use super::note::scan_links;
use super::{IndexedLink, LinkId, LinkIndex, LinkTarget};
use crate::index::EntryKind;
use crate::note_batch::NoteBatch;
use crate::parallel::in_parallel;
use crate::path::VaultPath;
use crate::refresh::VaultChange;

impl LinkIndex {
    /// Indexa `files`; `read` da el contenido de cada nota, o `None` si no se
    /// pudo leer y entonces queda sin links. Lee, escanea y resuelve las
    /// notas en varios hilos.
    pub fn build(
        files: impl IntoIterator<Item = (VaultPath, EntryKind)>,
        read: impl Fn(&VaultPath) -> Option<String> + Sync,
    ) -> Self {
        let files: Vec<(VaultPath, EntryKind)> = files.into_iter().collect();
        let notes: Vec<VaultPath> = files
            .iter()
            .filter(|(_, kind)| *kind == EntryKind::Note)
            .map(|(path, _)| path.clone())
            .collect();
        let scans = in_parallel(notes, |path| {
            let links = read(&path).as_deref().map(scan_links).unwrap_or_default();
            (path, links)
        });
        Self::from_scans(files, scans)
    }

    /// Indexa `files` con los links ya escaneados de cada nota: los resuelve
    /// en varios hilos sin leer nada.
    pub(crate) fn from_scans(
        files: impl IntoIterator<Item = (VaultPath, EntryKind)>,
        scans: Vec<(VaultPath, Vec<IndexedLink>)>,
    ) -> Self {
        let mut index = Self::default();
        for (path, kind) in files {
            if kind != EntryKind::Folder {
                index.names.insert(&path);
            }
        }
        let names = &index.names;
        let resolved = in_parallel(scans, |(path, mut links)| {
            for link in &mut links {
                link.target = names.resolve(&path, &link.target_text);
            }
            (path, links)
        });
        index.register_all(resolved);
        index
    }

    /// Aplica un cambio del watcher. `notes` da lo escaneado de una nota creada
    /// o modificada.
    pub fn apply(&mut self, change: &VaultChange, notes: &mut NoteBatch<'_>) {
        match change {
            VaultChange::Created {
                kind: EntryKind::Folder,
                ..
            } => {}
            VaultChange::Created { path, .. } => self.add_file(path, notes),
            VaultChange::Removed { path, .. } => {
                let mut keys = HashSet::new();
                for file in self.names.within(path) {
                    self.remove_note(&file);
                    keys.extend(self.names.remove(&file));
                }
                self.resolve_again(&keys);
            }
            VaultChange::Renamed { from, to, .. } => self.rename(from, to, notes),
            VaultChange::Modified { path } => {
                if self.notes.contains_key(path) {
                    self.replace_links(path, notes.scan(path).links.clone());
                }
            }
        }
    }

    /// Vuelve a leer las notas en `path` (y dentro, si es una carpeta) sin
    /// que el vault haya cambiado: un chmod puede volver legible o ilegible
    /// una nota. Devuelve si cambió algún link.
    pub(crate) fn reread(&mut self, path: &VaultPath, notes: &mut NoteBatch<'_>) -> bool {
        let mut changed = false;
        for note in self.names.within(path) {
            let Some(before) = self.notes.get(&note).cloned() else {
                continue;
            };
            self.replace_links(&note, notes.scan(&note).links.clone());
            changed |= self.notes.get(&note) != Some(&before);
        }
        changed
    }

    fn add_file(&mut self, path: &VaultPath, notes: &mut NoteBatch<'_>) {
        self.remove_note(path);
        let mut keys: HashSet<String> = self.names.remove(path).into_iter().collect();
        keys.extend(self.names.insert(path));
        if EntryKind::of_file(path) == EntryKind::Note {
            self.add_note(path.clone(), notes.scan(path).links.clone());
        }
        self.resolve_again(&keys);
    }

    /// Mueve `from`, y todo lo que contiene si es una carpeta, a `to`. Los
    /// links de una nota movida se vuelven a resolver desde su ruta nueva.
    fn rename(&mut self, from: &VaultPath, to: &VaultPath, notes: &mut NoteBatch<'_>) {
        let mut keys = HashSet::new();
        let mut moved = Vec::new();
        for old in self.names.within(from) {
            let Some(new) = old.rebase(from, to) else {
                continue;
            };
            let links = self.remove_note(&old);
            keys.extend(self.names.remove(&old));
            moved.push((new, links));
        }
        for (new, links) in moved {
            keys.extend(self.names.insert(&new));
            if EntryKind::of_file(&new) == EntryKind::Note {
                let links = match links {
                    Some(links) => Arc::unwrap_or_clone(links),
                    None => notes.scan(&new).links.clone(),
                };
                self.add_note(new, links);
            }
        }
        self.resolve_again(&keys);
    }

    fn replace_links(&mut self, path: &VaultPath, links: Vec<IndexedLink>) {
        self.remove_note(path);
        self.add_note(path.clone(), links);
    }

    fn add_note(&mut self, path: VaultPath, mut links: Vec<IndexedLink>) {
        for link in &mut links {
            link.target = self.names.resolve(&path, &link.target_text);
        }
        self.register(path, links);
    }

    /// Mete los links de `path`, ya resueltos.
    fn register(&mut self, path: VaultPath, links: Vec<IndexedLink>) {
        for (ordinal, link) in links.iter().enumerate() {
            if let Some(key) = key_of(&path, &link.target_text) {
                self.by_key.entry(key).or_default().insert(path.clone());
            }
            add_incoming(&mut self.incoming, &path, ordinal, &link.target);
        }
        self.notes.insert(path, Arc::new(links));
    }

    /// Como [`LinkIndex::register`] con cada nota, pero toca cada entrada de los
    /// mapas persistentes una sola vez: con un vault entero se nota al abrir.
    fn register_all(&mut self, notes: Vec<(VaultPath, Vec<IndexedLink>)>) {
        let mut by_key: StdHashMap<String, Vec<VaultPath>> = StdHashMap::new();
        let mut incoming: StdHashMap<VaultPath, Vec<LinkId>> = StdHashMap::new();
        for (path, links) in notes {
            for (ordinal, link) in links.iter().enumerate() {
                if let Some(key) = key_of(&path, &link.target_text) {
                    by_key.entry(key).or_default().push(path.clone());
                }
                if let LinkTarget::Resolved(target) = &link.target {
                    let id = LinkId {
                        source: path.clone(),
                        ordinal,
                    };
                    incoming.entry(target.clone()).or_default().push(id);
                }
            }
            self.notes.insert(path, Arc::new(links));
        }
        self.by_key = by_key
            .into_iter()
            .map(|(key, paths)| (key, paths.into_iter().collect()))
            .collect();
        self.incoming = incoming
            .into_iter()
            .map(|(target, ids)| (target, ids.into_iter().collect()))
            .collect();
    }

    fn remove_note(&mut self, path: &VaultPath) -> Option<Arc<Vec<IndexedLink>>> {
        let links = self.notes.remove(path)?;
        for (ordinal, link) in links.iter().enumerate() {
            if let Some(key) = key_of(path, &link.target_text) {
                remove_from(&mut self.by_key, &key, path);
            }
            remove_incoming(&mut self.incoming, path, ordinal, &link.target);
        }
        Some(links)
    }

    /// Vuelve a resolver los links a alguna de `keys`.
    fn resolve_again(&mut self, keys: &HashSet<String>) {
        let sources: BTreeSet<VaultPath> = keys
            .iter()
            .filter_map(|key| self.by_key.get(key))
            .flatten()
            .cloned()
            .collect();
        for source in sources {
            let Some(links) = self.notes.get_mut(&source) else {
                continue;
            };
            for (ordinal, link) in Arc::make_mut(links).iter_mut().enumerate() {
                if !key_of(&source, &link.target_text).is_some_and(|key| keys.contains(&key)) {
                    continue;
                }
                let target = self.names.resolve(&source, &link.target_text);
                if target != link.target {
                    remove_incoming(&mut self.incoming, &source, ordinal, &link.target);
                    add_incoming(&mut self.incoming, &source, ordinal, &target);
                    link.target = target;
                }
            }
        }
    }
}

fn add_incoming(
    incoming: &mut HashMap<VaultPath, OrdSet<LinkId>>,
    source: &VaultPath,
    ordinal: usize,
    target: &LinkTarget,
) {
    if let LinkTarget::Resolved(target) = target {
        incoming.entry(target.clone()).or_default().insert(LinkId {
            source: source.clone(),
            ordinal,
        });
    }
}

fn remove_incoming(
    incoming: &mut HashMap<VaultPath, OrdSet<LinkId>>,
    source: &VaultPath,
    ordinal: usize,
    target: &LinkTarget,
) {
    if let LinkTarget::Resolved(target) = target {
        remove_from(
            incoming,
            target,
            &LinkId {
                source: source.clone(),
                ordinal,
            },
        );
    }
}

/// Quita `value` del conjunto de `key` y el conjunto si queda vacío, para
/// que el índice incremental y el armado de cero sean iguales.
fn remove_from<K, V>(map: &mut HashMap<K, OrdSet<V>>, key: &K, value: &V)
where
    K: std::hash::Hash + Eq + Clone,
    V: Ord + Clone,
{
    if let Some(set) = map.get_mut(key) {
        set.remove(value);
        if set.is_empty() {
            map.remove(key);
        }
    }
}
