use std::collections::{BTreeSet, HashMap};

use crate::path::VaultPath;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EntryKind {
    Folder,
    /// Un `.md`.
    Note,
    /// Cualquier otro archivo: imágenes, PDFs, …
    Attachment,
}

impl EntryKind {
    pub(crate) fn of_file(path: &VaultPath) -> Self {
        let is_markdown = path
            .name()
            .rsplit_once('.')
            .is_some_and(|(stem, ext)| !stem.is_empty() && ext.eq_ignore_ascii_case("md"));
        if is_markdown {
            Self::Note
        } else {
            Self::Attachment
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry<'a> {
    pub path: &'a VaultPath,
    pub kind: EntryKind,
}

/// Los archivos y carpetas visibles del vault, como árbol.
///
/// Es un valor inmutable para quien lo lee: el watcher arma uno nuevo y lo
/// publica entero (ver [`crate::Vault::snapshot`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultIndex {
    entries: HashMap<VaultPath, EntryKind>,
    children: HashMap<VaultPath, BTreeSet<ChildKey>>,
}

/// Orden de los hijos en el árbol, como el "A a Z" de Obsidian: carpetas
/// primero y luego por nombre sin distinguir mayúsculas.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ChildKey {
    is_file: bool,
    folded: String,
    path: VaultPath,
    kind: EntryKind,
}

impl ChildKey {
    fn new(path: &VaultPath, kind: EntryKind) -> Self {
        Self {
            is_file: kind != EntryKind::Folder,
            folded: path.name().to_lowercase(),
            path: path.clone(),
            kind,
        }
    }
}

impl Default for VaultIndex {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            children: HashMap::from([(VaultPath::root(), BTreeSet::new())]),
        }
    }
}

impl VaultIndex {
    /// Arma el índice en cualquier orden: cada carpeta entra antes que su contenido.
    pub(crate) fn from_entries(entries: Vec<(VaultPath, EntryKind)>) -> Self {
        let mut index = Self::default();
        index.insert_all(entries);
        index
    }

    /// `Folder` para la raíz.
    #[must_use]
    pub fn kind(&self, path: &VaultPath) -> Option<EntryKind> {
        if path.is_root() {
            return Some(EntryKind::Folder);
        }
        self.entries.get(path).copied()
    }

    #[must_use]
    pub fn contains(&self, path: &VaultPath) -> bool {
        self.kind(path).is_some()
    }

    /// Cuántos archivos y carpetas hay, sin contar la raíz.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Todo lo indexado, sin orden.
    pub fn iter(&self) -> impl Iterator<Item = Entry<'_>> {
        self.entries
            .iter()
            .map(|(path, &kind)| Entry { path, kind })
    }

    /// Las notas y adjuntos, sin orden: lo que busca el finder.
    pub fn files(&self) -> impl Iterator<Item = Entry<'_>> {
        self.iter().filter(|entry| entry.kind != EntryKind::Folder)
    }

    /// El contenido directo de una carpeta, en el orden del árbol. Vacío si
    /// `dir` no es una carpeta del índice.
    pub fn children(&self, dir: &VaultPath) -> impl Iterator<Item = Entry<'_>> {
        self.children
            .get(dir)
            .into_iter()
            .flatten()
            .map(|key| Entry {
                path: &key.path,
                kind: key.kind,
            })
    }

    /// `path` y todo lo que contiene, si está en el índice.
    pub(crate) fn subtree(&self, path: &VaultPath) -> HashMap<VaultPath, EntryKind> {
        let mut found = HashMap::new();
        self.collect_subtree(path, &mut found);
        found
    }

    fn collect_subtree(&self, path: &VaultPath, found: &mut HashMap<VaultPath, EntryKind>) {
        if let Some(kind) = self.entries.get(path) {
            found.insert(path.clone(), *kind);
        }
        for child in self.children.get(path).into_iter().flatten() {
            self.collect_subtree(&child.path, found);
        }
    }

    pub(crate) fn insert_all(&mut self, mut entries: Vec<(VaultPath, EntryKind)>) {
        entries.sort_by_cached_key(|(path, _)| path.depth());
        for (path, kind) in entries {
            self.insert(path, kind);
        }
    }

    /// Mete `path`; si ya estaba con otro tipo, lo reemplaza con todo lo que tenía dentro.
    /// Su carpeta tiene que estar en el índice.
    fn insert(&mut self, path: VaultPath, kind: EntryKind) {
        match self.entries.get(&path) {
            Some(&old) if old == kind => return,
            Some(_) => self.remove_subtree(&path),
            None => {}
        }
        let Some(parent) = path.parent() else {
            return;
        };
        let Some(siblings) = self.children.get_mut(&parent) else {
            log::warn!("{path} llegó sin su carpeta al índice");
            return;
        };
        siblings.insert(ChildKey::new(&path, kind));
        if kind == EntryKind::Folder {
            self.children.insert(path.clone(), BTreeSet::new());
        }
        self.entries.insert(path, kind);
    }

    /// Quita `path` y lo que contiene. En la raíz, la vacía.
    pub(crate) fn remove_subtree(&mut self, path: &VaultPath) {
        if path.is_root() {
            *self = Self::default();
            return;
        }
        let Some(kind) = self.entries.remove(path) else {
            return;
        };
        if let Some(parent) = path.parent()
            && let Some(siblings) = self.children.get_mut(&parent)
        {
            siblings.remove(&ChildKey::new(path, kind));
        }
        for child in self.children.remove(path).into_iter().flatten() {
            self.remove_subtree(&child.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use pretty_assertions::assert_eq;

    use super::*;

    fn path(path: &str) -> VaultPath {
        VaultPath::new(path).unwrap()
    }

    fn index(entries: &[(&str, EntryKind)]) -> VaultIndex {
        VaultIndex::from_entries(entries.iter().map(|&(p, kind)| (path(p), kind)).collect())
    }

    fn names(index: &VaultIndex, dir: &str) -> Vec<String> {
        index
            .children(&path(dir))
            .map(|entry| entry.path.name().to_owned())
            .collect()
    }

    /// Las rutas de `nota_perf::vault_paths` con sus carpetas.
    fn vault_entries(notes: usize) -> Vec<(VaultPath, EntryKind)> {
        let files: Vec<_> = nota_perf::vault_paths(notes)
            .iter()
            .map(|file| path(file))
            .collect();
        let folders: HashSet<_> = files
            .iter()
            .flat_map(|file| std::iter::successors(file.parent(), VaultPath::parent))
            .filter(|folder| !folder.is_root())
            .collect();
        folders
            .into_iter()
            .map(|folder| (folder, EntryKind::Folder))
            .chain(files.into_iter().map(|file| {
                let kind = EntryKind::of_file(&file);
                (file, kind)
            }))
            .collect()
    }

    #[test]
    fn indexar_es_lineal() {
        nota_perf::assert_linear(1_000, vault_entries, VaultIndex::from_entries);
    }

    #[test]
    fn quitar_todo_el_contenido_es_lineal() {
        nota_perf::assert_linear(
            2_000,
            |notes| VaultIndex::from_entries(vault_entries(notes)),
            |mut index| {
                for top in 0..10 {
                    index.remove_subtree(&path(&format!("area-{top}")));
                }
                index
            },
        );
    }

    #[test]
    fn el_tipo_sale_de_la_extension() {
        assert_eq!(EntryKind::of_file(&path("a/nota.md")), EntryKind::Note);
        assert_eq!(EntryKind::of_file(&path("NOTA.MD")), EntryKind::Note);
        assert_eq!(EntryKind::of_file(&path("img.png")), EntryKind::Attachment);
        assert_eq!(EntryKind::of_file(&path("md")), EntryKind::Attachment);
        assert_eq!(
            EntryKind::of_file(&path("nota.md.tmp")),
            EntryKind::Attachment
        );
    }

    #[test]
    fn hijos_con_carpetas_primero_y_sin_mayusculas() {
        let index = index(&[
            ("b.md", EntryKind::Note),
            ("Zeta", EntryKind::Folder),
            ("A.md", EntryKind::Note),
            ("alfa", EntryKind::Folder),
            ("alfa/x.png", EntryKind::Attachment),
        ]);
        assert_eq!(names(&index, ""), ["alfa", "Zeta", "A.md", "b.md"]);
        assert_eq!(names(&index, "alfa"), ["x.png"]);
        assert_eq!(names(&index, "alfa/x.png"), Vec::<String>::new());
        assert_eq!(index.len(), 5);
    }

    #[test]
    fn el_orden_de_entrada_no_importa() {
        let entries = [
            ("a/b/c.md", EntryKind::Note),
            ("a/b", EntryKind::Folder),
            ("a", EntryKind::Folder),
        ];
        let mut reversed = entries;
        reversed.reverse();
        assert_eq!(index(&entries), index(&reversed));
        assert_eq!(index(&entries).len(), 3);
    }

    #[test]
    fn quitar_una_carpeta_quita_su_contenido() {
        let mut index = index(&[
            ("a", EntryKind::Folder),
            ("a/b", EntryKind::Folder),
            ("a/b/c.md", EntryKind::Note),
            ("d.md", EntryKind::Note),
        ]);
        index.remove_subtree(&path("a"));
        assert_eq!(index, self::index(&[("d.md", EntryKind::Note)]));
    }

    #[test]
    fn cambiar_de_tipo_reemplaza_el_subarbol() {
        let mut index = index(&[("a", EntryKind::Folder), ("a/c.md", EntryKind::Note)]);
        index.insert_all(vec![(path("a"), EntryKind::Attachment)]);
        assert_eq!(index, self::index(&[("a", EntryKind::Attachment)]));
    }
}
