//! En qué orden salen los cambios de un lote, para que quien los aplique
//! sobre un árbol nunca toque una carpeta que no existe ni pise un nombre.

use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::Bound;

use crate::index::EntryKind;
use crate::path::VaultPath;
use crate::refresh::VaultChange;

pub(crate) type Entries = BTreeMap<VaultPath, EntryKind>;

/// Los cambios de un lote, en orden de aplicación. Los renames que se cruzan
/// (uno ocupa lo que el otro libera y al revés) no tienen orden posible: el
/// lote sale entonces como borrar y crear, que siempre se puede aplicar.
pub(crate) fn arrange(
    before: &HashMap<VaultPath, EntryKind>,
    after: &HashMap<VaultPath, EntryKind>,
    removed: &Entries,
    created: &Entries,
    renames: &[(VaultPath, VaultPath)],
) -> Vec<VaultChange> {
    let mut paired_removed = removed.clone();
    let mut paired_created = created.clone();
    let moves = pair_renames(renames, &mut paired_removed, &mut paired_created);
    let changes = |removed: Entries, created: Entries, moves: Vec<VaultChange>| {
        let mut changes: Vec<_> = removed
            .into_iter()
            .map(|(path, kind)| VaultChange::Removed { path, kind })
            .chain(moves)
            .chain(
                created
                    .into_iter()
                    .map(|(path, kind)| VaultChange::Created { path, kind }),
            )
            .collect();
        changes.sort_by_cached_key(priority);
        changes
    };
    let mut tree = Tree::new(before, after);
    if let Some(ordered) = tree.schedule(changes(paired_removed, paired_created, moves)) {
        return ordered;
    }
    // Sin renames nada se cruza: borrar de adentro hacia afuera y crear de
    // afuera hacia adentro siempre se puede.
    changes(removed.clone(), created.clone(), Vec::new())
}

/// Primero lo que se borra, de lo más hondo hacia afuera; luego lo que
/// aparece, de afuera hacia adentro, moviendo antes de crear.
fn priority(change: &VaultChange) -> (u8, Reverse<usize>, usize, VaultPath) {
    match change {
        VaultChange::Removed { path, .. } => (0, Reverse(path.depth()), 0, path.clone()),
        VaultChange::Renamed { to, .. } => (1, Reverse(0), to.depth(), to.clone()),
        VaultChange::Created { path, .. } | VaultChange::Modified { path } => {
            (2, Reverse(0), path.depth(), path.clone())
        }
    }
}

/// Lo que hay en el árbol mientras se aplican los cambios. Solo sigue las
/// rutas del lote; lo de fuera no cambia y existe.
struct Tree {
    present: BTreeMap<VaultPath, EntryKind>,
    tracked: HashSet<VaultPath>,
    children: HashMap<VaultPath, usize>,
}

impl Tree {
    fn new(before: &HashMap<VaultPath, EntryKind>, after: &HashMap<VaultPath, EntryKind>) -> Self {
        let mut tree = Self {
            present: BTreeMap::new(),
            tracked: before.keys().chain(after.keys()).cloned().collect(),
            children: HashMap::new(),
        };
        for (path, kind) in before {
            tree.insert(path.clone(), *kind);
        }
        tree
    }

    fn insert(&mut self, path: VaultPath, kind: EntryKind) {
        if let Some(parent) = path.parent() {
            *self.children.entry(parent).or_default() += 1;
        }
        self.tracked.insert(path.clone());
        self.present.insert(path, kind);
    }

    fn remove(&mut self, path: &VaultPath) -> Option<EntryKind> {
        let kind = self.present.remove(path)?;
        if let Some(parent) = path.parent()
            && let Some(count) = self.children.get_mut(&parent)
        {
            *count -= 1;
        }
        Some(kind)
    }

    fn exists(&self, path: &VaultPath) -> bool {
        path.is_root() || self.present.contains_key(path) || !self.tracked.contains(path)
    }

    fn is_folder(&self, path: &VaultPath) -> bool {
        match self.present.get(path) {
            Some(kind) => *kind == EntryKind::Folder,
            None => path.is_root() || !self.tracked.contains(path),
        }
    }

    fn has_children(&self, path: &VaultPath) -> bool {
        self.children.get(path).is_some_and(|&count| count > 0)
    }

    fn parent_exists(&self, path: &VaultPath) -> bool {
        path.parent().is_some_and(|parent| self.is_folder(&parent))
    }

    fn can_apply(&self, change: &VaultChange) -> bool {
        match change {
            VaultChange::Removed { path, .. } => self.exists(path) && !self.has_children(path),
            VaultChange::Created { path, .. } => !self.exists(path) && self.parent_exists(path),
            VaultChange::Renamed { from, to, .. } => {
                self.exists(from) && !self.exists(to) && self.parent_exists(to)
            }
            VaultChange::Modified { .. } => true,
        }
    }

    fn apply(&mut self, change: &VaultChange) {
        match change {
            VaultChange::Removed { path, .. } => {
                self.remove(path);
            }
            VaultChange::Created { path, kind } => self.insert(path.clone(), *kind),
            VaultChange::Renamed { from, to, .. } => {
                let moved: Vec<_> = subtree(&self.present, from)
                    .map(|(path, _)| path.clone())
                    .collect();
                let moved: Vec<_> = moved
                    .into_iter()
                    .filter_map(|path| Some((path.rebase(from, to)?, self.remove(&path)?)))
                    .collect();
                for (target, kind) in moved {
                    self.insert(target, kind);
                }
            }
            VaultChange::Modified { .. } => {}
        }
    }

    /// En pasadas: cada una aplica, en orden, todo lo que ya se puede y deja
    /// el resto para la siguiente. `None` si una pasada no avanza.
    fn schedule(&mut self, mut pending: Vec<VaultChange>) -> Option<Vec<VaultChange>> {
        let mut ordered = Vec::with_capacity(pending.len());
        while !pending.is_empty() {
            let waiting = pending.len();
            let mut blocked = Vec::new();
            for change in pending {
                if self.can_apply(&change) {
                    self.apply(&change);
                    ordered.push(change);
                } else {
                    blocked.push(change);
                }
            }
            if blocked.len() == waiting {
                return None;
            }
            pending = blocked;
        }
        Some(ordered)
    }
}

/// Convierte en `Renamed` lo que se borró en `from` y apareció igual en `to`.
fn pair_renames(
    renames: &[(VaultPath, VaultPath)],
    removed: &mut Entries,
    created: &mut Entries,
) -> Vec<VaultChange> {
    let mut changes = Vec::new();
    for (from, to) in renames {
        let Some(&kind) = removed
            .get(from)
            .filter(|&kind| created.get(to) == Some(kind))
        else {
            continue;
        };
        let moved: Vec<_> = subtree(removed, from)
            .filter_map(|(path, kind)| {
                let target = path.rebase(from, to)?;
                (created.get(&target) == Some(kind)).then(|| (path.clone(), target))
            })
            .collect();
        for (old, new) in moved {
            removed.remove(&old);
            created.remove(&new);
        }
        changes.push(VaultChange::Renamed {
            from: from.clone(),
            to: to.clone(),
            kind,
        });
    }
    changes
}

/// `root` y lo que tiene dentro. En orden de cadena, el contenido de `a` va
/// seguido desde `a/`, así que es un rango y no hace falta recorrer el resto.
fn subtree<'a, V>(
    entries: &'a BTreeMap<VaultPath, V>,
    root: &VaultPath,
) -> impl Iterator<Item = (&'a VaultPath, &'a V)> {
    let inside = format!("{root}/");
    let descendants = entries.range::<str, _>((Bound::Included(inside.as_str()), Bound::Unbounded));
    entries
        .get_key_value(root)
        .into_iter()
        .chain(descendants.take_while(move |(path, _)| path.as_str().starts_with(&inside)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(path: &str) -> VaultPath {
        VaultPath::new(path).unwrap()
    }

    /// Borrar desde fuera de la app la carpeta `grande` con `notes` notas.
    fn folder_removal(notes: usize) -> (HashMap<VaultPath, EntryKind>, Entries) {
        let mut before = HashMap::new();
        let mut removed = Entries::new();
        before.insert(path("grande"), EntryKind::Folder);
        removed.insert(path("grande"), EntryKind::Folder);
        for note in 0..notes {
            let note = path(&format!("grande/n{note}.md"));
            before.insert(note.clone(), EntryKind::Note);
            removed.insert(note, EntryKind::Note);
        }
        (before, removed)
    }

    /// Un lote con `notes` renames independientes (`n0 → m0`, `n1 → m1`, …),
    /// como llega del watcher: cada origen borrado y cada destino creado.
    struct RenameBatch {
        before: HashMap<VaultPath, EntryKind>,
        after: HashMap<VaultPath, EntryKind>,
        removed: Entries,
        created: Entries,
        renames: Vec<(VaultPath, VaultPath)>,
    }

    impl RenameBatch {
        fn new(notes: usize) -> Self {
            let mut batch = Self {
                before: HashMap::new(),
                after: HashMap::new(),
                removed: Entries::new(),
                created: Entries::new(),
                renames: Vec::new(),
            };
            for note in 0..notes {
                let from = path(&format!("n{note:06}.md"));
                let to = path(&format!("m{note:06}.md"));
                batch.before.insert(from.clone(), EntryKind::Note);
                batch.after.insert(to.clone(), EntryKind::Note);
                batch.renames.push((from, to));
            }
            for (path, &kind) in &batch.before {
                if !batch.after.contains_key(path) {
                    batch.removed.insert(path.clone(), kind);
                }
            }
            for (path, &kind) in &batch.after {
                if !batch.before.contains_key(path) {
                    batch.created.insert(path.clone(), kind);
                }
            }
            batch
        }

        fn arrange(&self) -> Vec<VaultChange> {
            arrange(
                &self.before,
                &self.after,
                &self.removed,
                &self.created,
                &self.renames,
            )
        }
    }

    /// Borrar una carpeta con 5 000 notas desde fuera de la app llega como un
    /// solo lote: los hijos salen antes que la carpeta.
    #[test]
    fn un_lote_grande_borra_los_hijos_antes_que_la_carpeta() {
        let (before, removed) = folder_removal(5_000);
        let changes = arrange(&before, &HashMap::new(), &removed, &Entries::new(), &[]);
        assert_eq!(changes.len(), 5_001);
        assert_eq!(
            changes.last(),
            Some(&VaultChange::Removed {
                path: path("grande"),
                kind: EntryKind::Folder
            })
        );
    }

    #[test]
    fn ordenar_un_borrado_grande_es_lineal() {
        metalmind_perf::assert_linear(1_000, folder_removal, |(before, removed)| {
            arrange(&before, &HashMap::new(), &removed, &Entries::new(), &[])
        });
    }

    #[test]
    fn ordenar_muchos_renames_es_lineal() {
        metalmind_perf::assert_linear(1_000, RenameBatch::new, |batch| batch.arrange());
    }

    /// La prueba de arriba mide algo solo si cada rename del lote se empareja.
    #[test]
    fn cada_rename_del_lote_sale_como_renamed() {
        let changes = RenameBatch::new(50).arrange();
        assert_eq!(changes.len(), 50);
        assert!(
            changes
                .iter()
                .all(|change| matches!(change, VaultChange::Renamed { .. }))
        );
    }
}
