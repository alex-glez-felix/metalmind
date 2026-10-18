//! En qué orden salen los cambios de un lote, para que quien los aplique
//! sobre un árbol nunca toque una carpeta que no existe ni pise un nombre.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::ops::Bound;

use crate::index::EntryKind;
use crate::path::VaultPath;
use crate::refresh::VaultChange;

pub(crate) type Entries = BTreeSet<(VaultPath, EntryKind)>;

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
                // En orden de cadena, el contenido de `a` va seguido desde `a/`.
                let inside = format!("{from}/");
                let moved: Vec<_> = std::iter::once(from.clone())
                    .chain(
                        self.present
                            .range::<str, _>((Bound::Included(inside.as_str()), Bound::Unbounded))
                            .map(|(path, _)| path)
                            .take_while(|path| path.as_str().starts_with(&inside))
                            .cloned(),
                    )
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
        let Some(kind) = [EntryKind::Folder, EntryKind::Note, EntryKind::Attachment]
            .into_iter()
            .find(|&kind| {
                removed.contains(&(from.clone(), kind)) && created.contains(&(to.clone(), kind))
            })
        else {
            continue;
        };
        let moved: Vec<_> = removed
            .iter()
            .filter(|(path, _)| path.starts_with(from))
            .filter_map(|(path, kind)| {
                let target = (path.rebase(from, to)?, *kind);
                created
                    .contains(&target)
                    .then(|| ((path.clone(), *kind), target))
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
        removed.insert((path("grande"), EntryKind::Folder));
        for note in 0..notes {
            let note = path(&format!("grande/n{note}.md"));
            before.insert(note.clone(), EntryKind::Note);
            removed.insert((note, EntryKind::Note));
        }
        (before, removed)
    }

    /// `notes` renames del lote; `chained` hace que cada uno ocupe el nombre
    /// que libera el siguiente (`n0 → n1`, `n1 → n2`, …).
    struct RenameBatch {
        before: HashMap<VaultPath, EntryKind>,
        after: HashMap<VaultPath, EntryKind>,
        removed: Entries,
        created: Entries,
        renames: Vec<(VaultPath, VaultPath)>,
    }

    impl RenameBatch {
        fn new(notes: usize, chained: bool) -> Self {
            let mut batch = Self {
                before: HashMap::new(),
                after: HashMap::new(),
                removed: Entries::new(),
                created: Entries::new(),
                renames: Vec::new(),
            };
            for note in 0..notes {
                let from = path(&format!("n{note:06}.md"));
                let to = if chained {
                    path(&format!("n{:06}.md", note + 1))
                } else {
                    path(&format!("m{note:06}.md"))
                };
                batch.before.insert(from.clone(), EntryKind::Note);
                batch.after.insert(to.clone(), EntryKind::Note);
                batch.renames.push((from, to));
            }
            for (path, &kind) in &batch.before {
                if !batch.after.contains_key(path) {
                    batch.removed.insert((path.clone(), kind));
                }
            }
            for (path, &kind) in &batch.after {
                if !batch.before.contains_key(path) {
                    batch.created.insert((path.clone(), kind));
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
        nota_perf::assert_linear(1_000, folder_removal, |(before, removed)| {
            arrange(&before, &HashMap::new(), &removed, &Entries::new(), &[])
        });
    }

    #[test]
    #[ignore = "cuadrático en los renames del lote: #38"]
    fn ordenar_muchos_renames_es_lineal() {
        nota_perf::assert_linear(
            1_000,
            |notes| RenameBatch::new(notes, false),
            |batch| batch.arrange(),
        );
    }

    #[test]
    fn ordenar_una_cadena_de_renames_es_lineal() {
        nota_perf::assert_linear(
            1_200,
            |notes| RenameBatch::new(notes, true),
            |batch| batch.arrange(),
        );
    }
}
