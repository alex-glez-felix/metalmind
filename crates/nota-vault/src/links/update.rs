//! Armar el índice y mantenerlo al día con los cambios del vault. Cada
//! resolución es función de los archivos del vault y de la nota de origen,
//! así que aplicar cambios llega al mismo índice que armarlo de cero.

use std::collections::{BTreeSet, HashMap};
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::thread;

use super::names::key_of;
use super::note::scan_links;
use super::{IndexedLink, LinkId, LinkIndex, LinkTarget};
use crate::index::EntryKind;
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
        let mut index = Self::default();
        let mut notes = Vec::new();
        for (path, kind) in files {
            match kind {
                EntryKind::Folder => continue,
                EntryKind::Note => notes.push(path.clone()),
                EntryKind::Attachment => {}
            }
            index.names.insert(&path);
        }
        let names = &index.names;
        let resolved = in_parallel(notes, |path| {
            let mut links = read(&path).as_deref().map(scan_links).unwrap_or_default();
            for link in &mut links {
                link.target = names.resolve(&path, &link.target_text);
            }
            (path, links)
        });
        for (path, links) in resolved {
            index.register(path, links);
        }
        index
    }

    /// Aplica un cambio del watcher. `read` da el contenido actual de una
    /// nota creada o modificada.
    pub fn apply(
        &mut self,
        change: &VaultChange,
        mut read: impl FnMut(&VaultPath) -> Option<String>,
    ) {
        match change {
            VaultChange::Created {
                kind: EntryKind::Folder,
                ..
            } => {}
            VaultChange::Created { path, .. } => self.add_file(path, &mut read),
            VaultChange::Removed { path, .. } => {
                for file in self.names.within(path) {
                    self.remove_file(&file);
                }
            }
            VaultChange::Renamed { from, to, .. } => self.rename(from, to, &mut read),
            VaultChange::Modified { path } => {
                if self.notes.contains_key(path) {
                    let text = read(path);
                    self.replace_links(path, text.as_deref().map(scan_links).unwrap_or_default());
                }
            }
        }
    }

    fn add_file(&mut self, path: &VaultPath, read: &mut impl FnMut(&VaultPath) -> Option<String>) {
        self.remove_file(path);
        let keys = self.names.insert(path);
        if EntryKind::of_file(path) == EntryKind::Note {
            let links = read(path).as_deref().map(scan_links).unwrap_or_default();
            self.add_note(path.clone(), links);
        }
        self.resolve_again(&keys);
    }

    fn remove_file(&mut self, path: &VaultPath) {
        self.remove_note(path);
        let keys = self.names.remove(path);
        self.resolve_again(&keys);
    }

    /// Mueve `from`, y todo lo que contiene si es una carpeta, a `to`. Los
    /// links de una nota movida se vuelven a resolver desde su ruta nueva.
    fn rename(
        &mut self,
        from: &VaultPath,
        to: &VaultPath,
        read: &mut impl FnMut(&VaultPath) -> Option<String>,
    ) {
        let mut keys = Vec::new();
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
                    None => read(&new).as_deref().map(scan_links).unwrap_or_default(),
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

    /// Vuelve a resolver las notas con links a alguna de `keys`.
    fn resolve_again(&mut self, keys: &[String]) {
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

/// `work` sobre cada elemento, repartido en un hilo por núcleo, con los
/// resultados en el orden de `items`.
fn in_parallel<T: Send, R: Send>(items: Vec<T>, work: impl Fn(T) -> R + Sync) -> Vec<R> {
    let threads = thread::available_parallelism().map_or(1, NonZeroUsize::get);
    let chunk = items.len().div_ceil(threads).max(1);
    let mut chunks = Vec::new();
    let mut items = items.into_iter();
    loop {
        let part: Vec<T> = items.by_ref().take(chunk).collect();
        if part.is_empty() {
            break;
        }
        chunks.push(part);
    }
    let work = &work;
    thread::scope(|scope| {
        let handles: Vec<_> = chunks
            .into_iter()
            .map(|part| scope.spawn(move || part.into_iter().map(work).collect::<Vec<R>>()))
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect()
    })
}

fn add_incoming(
    incoming: &mut HashMap<VaultPath, BTreeSet<LinkId>>,
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
    incoming: &mut HashMap<VaultPath, BTreeSet<LinkId>>,
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
fn remove_from<K, V>(map: &mut HashMap<K, BTreeSet<V>>, key: &K, value: &V)
where
    K: std::hash::Hash + Eq,
    V: Ord,
{
    if let Some(set) = map.get_mut(key) {
        set.remove(value);
        if set.is_empty() {
            map.remove(key);
        }
    }
}
