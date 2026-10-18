//! Actualizar el índice a partir de las rutas que tocó un lote de eventos.
//!
//! No se interpreta cada evento: las rutas tocadas se vuelven a leer del
//! disco y los cambios salen de comparar antes y después. Por eso el resultado
//! es el mismo que un walk completo, lleguen los eventos como lleguen.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use crate::index::{EntryKind, VaultIndex};
use crate::path::VaultPath;
use crate::scan::scan_partial;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VaultChange {
    Created {
        path: VaultPath,
        kind: EntryKind,
    },
    Removed {
        path: VaultPath,
        kind: EntryKind,
    },
    /// Si es una carpeta, cubre todo lo que tenía dentro.
    Renamed {
        from: VaultPath,
        to: VaultPath,
        kind: EntryKind,
    },
    /// El contenido del archivo cambió, también si se reemplazó con un
    /// guardado atómico (temporal + rename).
    Modified {
        path: VaultPath,
    },
}

/// Lo que un lote de eventos dice que hay que volver a mirar.
#[derive(Debug, Default)]
pub(crate) struct Touched {
    pub paths: BTreeSet<VaultPath>,
    /// Pares `(de, a)` que el debouncer pudo emparejar.
    pub renames: Vec<(VaultPath, VaultPath)>,
}

/// Relee del disco lo que dice `touched` y devuelve qué cambió.
pub(crate) fn refresh(index: &mut VaultIndex, root: &Path, touched: &Touched) -> Vec<VaultChange> {
    let dirty = dirty_roots(index, &touched.paths);
    let before = subtrees(index, &dirty);
    for path in &dirty {
        index.remove_subtree(path);
    }
    index.insert_all(scan_partial(root, &dirty));
    let after = subtrees(index, &dirty);
    diff(&before, after, touched)
}

/// Las rutas a releer, sin las que ya cubre otra. Si la carpeta de una ruta
/// no está en el índice, se relee desde el primer ancestro que sí está.
fn dirty_roots(index: &VaultIndex, paths: &BTreeSet<VaultPath>) -> Vec<VaultPath> {
    let promoted: BTreeSet<VaultPath> = paths
        .iter()
        .map(|path| outermost_missing(index, path))
        .collect();
    // En orden, un ancestro va antes que lo que contiene.
    let mut roots: Vec<VaultPath> = Vec::new();
    for path in promoted {
        if !roots.iter().any(|root| path.starts_with(root)) {
            roots.push(path);
        }
    }
    roots
}

fn outermost_missing(index: &VaultIndex, path: &VaultPath) -> VaultPath {
    let mut outermost = path.clone();
    for ancestor in path.ancestors() {
        if index.kind(&ancestor) == Some(EntryKind::Folder) {
            break;
        }
        outermost = ancestor;
    }
    outermost
}

fn subtrees(index: &VaultIndex, roots: &[VaultPath]) -> HashMap<VaultPath, EntryKind> {
    roots.iter().flat_map(|root| index.subtree(root)).collect()
}

fn diff(
    before: &HashMap<VaultPath, EntryKind>,
    after: HashMap<VaultPath, EntryKind>,
    touched: &Touched,
) -> Vec<VaultChange> {
    let mut removed: BTreeSet<(VaultPath, EntryKind)> = BTreeSet::new();
    let mut created: BTreeSet<(VaultPath, EntryKind)> = BTreeSet::new();
    let mut modified = Vec::new();
    for (path, kind) in before {
        match after.get(path) {
            Some(new) if new == kind => {
                if *kind != EntryKind::Folder && touched.paths.contains(path) {
                    modified.push(path.clone());
                }
            }
            _ => {
                removed.insert((path.clone(), *kind));
            }
        }
    }
    for (path, kind) in after {
        if before.get(&path) != Some(&kind) {
            created.insert((path, kind));
        }
    }
    let mut changes = pair_renames(&touched.renames, &mut removed, &mut created);
    changes.extend(
        removed
            .into_iter()
            .map(|(path, kind)| VaultChange::Removed { path, kind }),
    );
    let mut created: Vec<_> = created.into_iter().collect();
    created.sort_by_cached_key(|(path, _)| (path.depth(), path.clone()));
    changes.extend(
        created
            .into_iter()
            .map(|(path, kind)| VaultChange::Created { path, kind }),
    );
    modified.sort();
    changes.extend(
        modified
            .into_iter()
            .map(|path| VaultChange::Modified { path }),
    );
    changes
}

/// Convierte en `Renamed` lo que se borró en `from` y apareció igual en `to`.
fn pair_renames(
    renames: &[(VaultPath, VaultPath)],
    removed: &mut BTreeSet<(VaultPath, EntryKind)>,
    created: &mut BTreeSet<(VaultPath, EntryKind)>,
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
    use std::fs;

    use pretty_assertions::assert_eq;
    use proptest::prelude::*;
    use tempfile::TempDir;

    use super::*;
    use crate::scan::scan;

    #[derive(Debug, Clone)]
    enum Op {
        WriteFile(String),
        CreateDir(String),
        Remove(String),
        Rename(String, String),
    }

    fn op() -> impl Strategy<Value = Op> {
        let path =
            prop::collection::vec(prop::sample::select(vec!["a", "b", "x.md", "y.png"]), 1..=3)
                .prop_map(|parts| parts.join("/"));
        prop_oneof![
            path.clone().prop_map(Op::WriteFile),
            path.clone().prop_map(Op::CreateDir),
            path.clone().prop_map(Op::Remove),
            (path.clone(), path).prop_map(|(from, to)| Op::Rename(from, to)),
        ]
    }

    /// Aplica `op` en disco y devuelve lo que el watcher diría que tocó.
    fn apply(root: &Path, op: &Op) -> Touched {
        let at = |path: &str| root.join(path);
        let vault = |path: &str| VaultPath::new(path).unwrap();
        let mut touched = Touched::default();
        let _ = match op {
            Op::WriteFile(path) => {
                let _ = fs::create_dir_all(at(path).parent().unwrap());
                touched.paths.insert(vault(path));
                fs::write(at(path), "")
            }
            Op::CreateDir(path) => {
                touched.paths.insert(vault(path));
                fs::create_dir_all(at(path))
            }
            Op::Remove(path) => {
                touched.paths.insert(vault(path));
                fs::remove_file(at(path)).or_else(|_| fs::remove_dir_all(at(path)))
            }
            Op::Rename(from, to) => {
                let _ = fs::create_dir_all(at(to).parent().unwrap());
                touched.paths.extend([vault(from), vault(to)]);
                touched.renames.push((vault(from), vault(to)));
                fs::rename(at(from), at(to))
            }
        };
        touched
    }

    type Mirror = HashMap<VaultPath, EntryKind>;

    fn mirror(index: &VaultIndex) -> Mirror {
        index
            .iter()
            .map(|entry| (entry.path.clone(), entry.kind))
            .collect()
    }

    /// Lo que haría la UI con los cambios sobre su propia copia del índice.
    fn replay(mirror: &mut Mirror, changes: &[VaultChange]) {
        for change in changes {
            match change {
                VaultChange::Created { path, kind } => {
                    mirror.insert(path.clone(), *kind);
                }
                VaultChange::Removed { path, .. } => {
                    mirror.remove(path);
                }
                VaultChange::Renamed { from, to, .. } => {
                    let moved: Vec<_> = mirror
                        .keys()
                        .filter(|path| path.starts_with(from))
                        .cloned()
                        .collect();
                    for path in moved {
                        let kind = mirror.remove(&path).unwrap();
                        mirror.insert(path.rebase(from, to).unwrap(), kind);
                    }
                }
                VaultChange::Modified { .. } => {}
            }
        }
    }

    proptest! {
        /// Reindexar a pedazos da el mismo índice que un walk completo, y los
        /// cambios emitidos llevan de un índice al otro.
        #[test]
        fn incremental_igual_a_walk_completo(ops in prop::collection::vec(op(), 1..30)) {
            let dir = TempDir::new().unwrap();
            let root = dir.path().canonicalize().unwrap();
            let mut index = scan(&root);
            let mut copy = mirror(&index);
            for op in &ops {
                let touched = apply(&root, op);
                let changes = refresh(&mut index, &root, &touched);
                replay(&mut copy, &changes);
                prop_assert_eq!(&index, &scan(&root), "tras {:?}", op);
                prop_assert_eq!(&copy, &mirror(&index), "cambios de {:?}: {:?}", op, changes);
            }
        }
    }

    #[test]
    fn renombrar_una_carpeta_con_otro_contenido_en_destino() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        fs::create_dir_all(root.join("a")).unwrap();
        fs::write(root.join("a/x.md"), "").unwrap();
        let mut index = scan(&root);
        fs::rename(root.join("a"), root.join("b")).unwrap();
        fs::write(root.join("b/nuevo.md"), "").unwrap();
        let path = |path: &str| VaultPath::new(path).unwrap();
        let touched = Touched {
            paths: [path("a"), path("b"), path("b/nuevo.md")].into(),
            renames: vec![(path("a"), path("b"))],
        };
        assert_eq!(
            refresh(&mut index, &root, &touched),
            [
                VaultChange::Renamed {
                    from: path("a"),
                    to: path("b"),
                    kind: EntryKind::Folder
                },
                VaultChange::Created {
                    path: path("b/nuevo.md"),
                    kind: EntryKind::Note
                },
            ]
        );
    }
}
