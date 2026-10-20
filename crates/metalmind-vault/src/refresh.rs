//! Actualizar el índice a partir de las rutas que tocó un lote de eventos.
//!
//! No se interpreta cada evento: las rutas tocadas se vuelven a leer del
//! disco y los cambios salen de comparar antes y después. Por eso el resultado
//! es el mismo que un walk completo, lleguen los eventos como lleguen.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use crate::index::{EntryKind, VaultIndex};
use crate::order::{Entries, arrange};
use crate::path::VaultPath;
use crate::scan::{BeforeReading, scan_partial};

/// Un cambio del índice. Los de un lote van en un orden que se puede aplicar
/// sobre un árbol: nada se crea ni se mueve a una carpeta que todavía no
/// existe, nada pisa un nombre ocupado y ninguna carpeta se borra con hijos.
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
    /// Las rutas donde se escribió contenido: un archivo movido en el mismo
    /// lote también cuenta como modificado.
    pub written: BTreeSet<VaultPath>,
    /// Rutas a releer cuyo contenido no cambió (permisos, dueño): pueden hacer
    /// legible una carpeta, pero no son una modificación.
    pub reread: BTreeSet<VaultPath>,
}

impl Touched {
    pub fn merge(&mut self, other: Self) {
        self.paths.extend(other.paths);
        self.renames.extend(other.renames);
        self.written.extend(other.written);
        self.reread.extend(other.reread);
    }
}

/// Relee del disco lo que dice `touched` y devuelve qué cambió. Cada carpeta
/// que relee pasa antes por `on_folder`.
pub(crate) fn refresh(
    index: &mut VaultIndex,
    root: &Path,
    touched: &Touched,
    on_folder: Option<BeforeReading>,
) -> Vec<VaultChange> {
    let all: BTreeSet<VaultPath> = touched.paths.union(&touched.reread).cloned().collect();
    let dirty = dirty_roots(index, &all);
    let before = subtrees(index, &dirty);
    for path in &dirty {
        index.remove_subtree(path);
    }
    let found = scan_partial(root, &dirty, on_folder);
    index.insert_all(found.entries);
    // Lo que no se pudo leer se queda como estaba hasta el próximo evento.
    let kept: HashMap<VaultPath, EntryKind> = before
        .iter()
        .filter(|(path, _)| {
            !index.contains(path) && found.unreadable.iter().any(|dir| path.starts_with(dir))
        })
        .map(|(path, kind)| (path.clone(), *kind))
        .collect();
    index.insert_all(
        kept.iter()
            .map(|(path, kind)| (path.clone(), *kind))
            .collect(),
    );
    let after = subtrees(index, &dirty);
    diff(&before, &after, touched, &kept)
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
    after: &HashMap<VaultPath, EntryKind>,
    touched: &Touched,
    unread: &HashMap<VaultPath, EntryKind>,
) -> Vec<VaultChange> {
    let mut removed = Entries::new();
    let mut created = Entries::new();
    let mut modified = BTreeSet::new();
    for (path, kind) in before {
        match after.get(path) {
            Some(new) if new == kind => {
                if *kind != EntryKind::Folder
                    && touched.paths.contains(path)
                    && !unread.contains_key(path)
                {
                    modified.insert(path.clone());
                }
            }
            _ => {
                removed.insert(path.clone(), *kind);
            }
        }
    }
    for (path, kind) in after {
        if before.get(path) != Some(kind) {
            created.insert(path.clone(), *kind);
        }
    }
    let mut changes = arrange(before, after, &removed, &created, &touched.renames);
    // Un archivo que llega por rename y pisa otro también trae contenido nuevo.
    let replaced = touched.renames.iter().map(|(_, to)| to).filter(|to| {
        before
            .get(*to)
            .is_some_and(|kind| *kind != EntryKind::Folder)
    });
    let arrived: BTreeSet<VaultPath> = touched.written.iter().chain(replaced).cloned().collect();
    modified.extend(edited_while_moving(&changes, after, &arrived));
    changes.extend(
        modified
            .into_iter()
            .map(|path| VaultChange::Modified { path }),
    );
    changes
}

/// Los archivos que un `Renamed` lleva a su destino y cuyo contenido cambió
/// en el lote (se escribieron o les llegó otro por rename), antes o después
/// de moverse.
fn edited_while_moving(
    changes: &[VaultChange],
    after: &HashMap<VaultPath, EntryKind>,
    written: &BTreeSet<VaultPath>,
) -> Vec<VaultPath> {
    let mut edited = Vec::new();
    for change in changes {
        let VaultChange::Renamed { from, to, .. } = change else {
            continue;
        };
        edited.extend(
            after
                .iter()
                .filter(|(path, kind)| **kind != EntryKind::Folder && path.starts_with(to))
                .filter(|(path, _)| {
                    written.contains(*path)
                        || path
                            .rebase(to, from)
                            .is_some_and(|old| written.contains(&old))
                })
                .map(|(path, _)| path.clone()),
        );
    }
    edited
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
                touched.written.insert(vault(path));
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
            replay_one(mirror, change).unwrap_or_else(|error| panic!("{change:?}: {error}"));
        }
    }

    /// Como un árbol de verdad: no se crea ni se mueve nada a una carpeta que
    /// no existe, ni se pisa algo, ni se borra una carpeta con hijos.
    fn replay_one(mirror: &mut Mirror, change: &VaultChange) -> Result<(), &'static str> {
        let has_folder = |mirror: &Mirror, path: &VaultPath| {
            path.parent().is_some_and(|parent| {
                parent.is_root() || mirror.get(&parent) == Some(&EntryKind::Folder)
            })
        };
        match change {
            VaultChange::Created { path, kind } => {
                if !has_folder(mirror, path) {
                    return Err("su carpeta no existe");
                }
                if mirror.insert(path.clone(), *kind).is_some() {
                    return Err("pisa algo que existe");
                }
            }
            VaultChange::Removed { path, .. } => {
                if mirror
                    .keys()
                    .any(|other| other != path && other.starts_with(path))
                {
                    return Err("borra una carpeta con hijos");
                }
                if mirror.remove(path).is_none() {
                    return Err("borra algo que no existe");
                }
            }
            VaultChange::Renamed { from, to, .. } => {
                if !mirror.contains_key(from) {
                    return Err("mueve algo que no existe");
                }
                if mirror.contains_key(to) {
                    return Err("pisa algo que existe");
                }
                if !has_folder(mirror, to) {
                    return Err("su destino no tiene carpeta");
                }
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
            VaultChange::Modified { path } => {
                if !mirror.contains_key(path) {
                    return Err("modifica algo que no existe");
                }
            }
        }
        Ok(())
    }

    /// Reindexar a pedazos da el mismo índice que un walk completo, y los
    /// cambios emitidos llevan de un índice al otro. Varias operaciones
    /// pueden caer en un mismo lote, como cuando el debouncer las junta.
    fn incremental_matches_full_walk(batches: &[Vec<Op>]) -> Result<(), TestCaseError> {
        let dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let mut index = scan(&root, None);
        let mut copy = mirror(&index);
        for batch in batches {
            let mut touched = Touched::default();
            for op in batch {
                touched.merge(apply(&root, op));
            }
            let changes = refresh(&mut index, &root, &touched, None);
            replay(&mut copy, &changes);
            prop_assert_eq!(&index, &scan(&root, None), "tras {:?}", batch);
            prop_assert_eq!(
                &copy,
                &mirror(&index),
                "cambios de {:?}: {:?}",
                batch,
                changes
            );
        }
        Ok(())
    }

    proptest! {
        #[test]
        fn incremental_igual_a_walk_completo(
            batches in prop::collection::vec(prop::collection::vec(op(), 1..5), 1..12),
        ) {
            incremental_matches_full_walk(&batches)?;
        }
    }

    /// Lo encontró el proptest en CI: el rename `a/a → y.png` falla en disco
    /// (`y.png` ya tiene algo), pero el lote lo trae emparejado.
    #[test]
    fn un_rename_que_no_coincide_con_el_disco_no_se_lleva_el_contenido() {
        let write = |path: &str| Op::WriteFile(path.to_owned());
        let rename = |from: &str, to: &str| Op::Rename(from.to_owned(), to.to_owned());
        incremental_matches_full_walk(&[
            vec![write("y.png/x.md"), rename("y.png", "a/a")],
            vec![write("y.png/a"), rename("a/a", "y.png"), rename("a", "b/a")],
        ])
        .unwrap();
    }

    #[test]
    fn una_carpeta_que_no_se_puede_leer_conserva_lo_que_tenia() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        fs::create_dir_all(root.join("cerrada/hondo")).unwrap();
        fs::write(root.join("cerrada/hondo/a.md"), "").unwrap();
        fs::write(root.join("cerrada/b.md"), "").unwrap();
        let mut index = scan(&root, None);
        let snapshot = index.clone();
        fs::set_permissions(root.join("cerrada"), fs::Permissions::from_mode(0o000)).unwrap();
        let touched = Touched {
            paths: [VaultPath::new("cerrada/b.md").unwrap()].into(),
            renames: Vec::new(),
            ..Touched::default()
        };
        let changes = refresh(&mut index, &root, &touched, None);
        fs::set_permissions(root.join("cerrada"), fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(changes, []);
        assert_eq!(index, snapshot);
    }

    #[test]
    fn devolver_los_permisos_a_una_carpeta_indexa_su_contenido() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        fs::create_dir_all(root.join("cerrada")).unwrap();
        fs::write(root.join("cerrada/a.md"), "").unwrap();
        fs::set_permissions(root.join("cerrada"), fs::Permissions::from_mode(0o000)).unwrap();
        let mut index = scan(&root, None);
        fs::set_permissions(root.join("cerrada"), fs::Permissions::from_mode(0o755)).unwrap();
        let path = |path: &str| VaultPath::new(path).unwrap();
        let touched = Touched {
            reread: [path("cerrada")].into(),
            ..Touched::default()
        };
        assert_eq!(
            refresh(&mut index, &root, &touched, None),
            [VaultChange::Created {
                path: path("cerrada/a.md"),
                kind: EntryKind::Note
            }]
        );
        assert_eq!(index, scan(&root, None));
    }

    #[test]
    fn un_chmod_no_es_una_modificacion() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        fs::write(root.join("a.md"), "").unwrap();
        let mut index = scan(&root, None);
        let touched = Touched {
            reread: [VaultPath::new("a.md").unwrap()].into(),
            ..Touched::default()
        };
        assert_eq!(refresh(&mut index, &root, &touched, None), []);
    }

    #[test]
    fn editar_y_mover_en_un_lote_avisa_las_dos_cosas() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        fs::create_dir_all(root.join("a")).unwrap();
        fs::write(root.join("a/x.md"), "").unwrap();
        fs::write(root.join("a/y.md"), "").unwrap();
        let mut index = scan(&root, None);
        fs::write(root.join("a/x.md"), "editada").unwrap();
        fs::rename(root.join("a"), root.join("b")).unwrap();
        let path = |path: &str| VaultPath::new(path).unwrap();
        let touched = Touched {
            paths: [path("a/x.md"), path("a"), path("b")].into(),
            renames: vec![(path("a"), path("b"))],
            written: [path("a/x.md")].into(),
            ..Touched::default()
        };
        assert_eq!(
            refresh(&mut index, &root, &touched, None),
            [
                VaultChange::Renamed {
                    from: path("a"),
                    to: path("b"),
                    kind: EntryKind::Folder
                },
                VaultChange::Modified {
                    path: path("b/x.md")
                },
            ]
        );
    }

    #[test]
    fn pisar_con_un_rename_y_mover_la_carpeta_avisa_el_cambio() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        fs::create_dir_all(root.join("a")).unwrap();
        fs::write(root.join("a/x.md"), "vieja").unwrap();
        let mut index = scan(&root, None);
        fs::write(root.join("nueva.md"), "nueva").unwrap();
        fs::rename(root.join("nueva.md"), root.join("a/x.md")).unwrap();
        fs::rename(root.join("a"), root.join("b")).unwrap();
        let path = |path: &str| VaultPath::new(path).unwrap();
        let touched = Touched {
            paths: [path("nueva.md"), path("a/x.md"), path("a"), path("b")].into(),
            renames: vec![(path("nueva.md"), path("a/x.md")), (path("a"), path("b"))],
            written: [path("nueva.md")].into(),
            ..Touched::default()
        };
        assert_eq!(
            refresh(&mut index, &root, &touched, None),
            [
                VaultChange::Renamed {
                    from: path("a"),
                    to: path("b"),
                    kind: EntryKind::Folder
                },
                VaultChange::Modified {
                    path: path("b/x.md")
                },
            ]
        );
    }

    #[test]
    fn mover_a_una_carpeta_nueva_crea_la_carpeta_antes() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        fs::write(root.join("x.md"), "").unwrap();
        let mut index = scan(&root, None);
        let mut copy = mirror(&index);
        fs::create_dir(root.join("nueva")).unwrap();
        fs::rename(root.join("x.md"), root.join("nueva/x.md")).unwrap();
        let path = |path: &str| VaultPath::new(path).unwrap();
        let touched = Touched {
            paths: [path("nueva"), path("x.md"), path("nueva/x.md")].into(),
            renames: vec![(path("x.md"), path("nueva/x.md"))],
            ..Touched::default()
        };
        let changes = refresh(&mut index, &root, &touched, None);
        assert_eq!(
            changes,
            [
                VaultChange::Created {
                    path: path("nueva"),
                    kind: EntryKind::Folder
                },
                VaultChange::Renamed {
                    from: path("x.md"),
                    to: path("nueva/x.md"),
                    kind: EntryKind::Note
                },
            ]
        );
        replay(&mut copy, &changes);
        assert_eq!(copy, mirror(&index));
    }

    #[test]
    fn borrar_un_hijo_y_renombrar_su_carpeta_en_un_lote() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        fs::create_dir_all(root.join("a")).unwrap();
        fs::write(root.join("a/x.md"), "").unwrap();
        fs::write(root.join("a/y.md"), "").unwrap();
        let mut index = scan(&root, None);
        let mut copy = mirror(&index);
        fs::remove_file(root.join("a/x.md")).unwrap();
        fs::rename(root.join("a"), root.join("b")).unwrap();
        let path = |path: &str| VaultPath::new(path).unwrap();
        let touched = Touched {
            paths: [path("a/x.md"), path("a"), path("b")].into(),
            renames: vec![(path("a"), path("b"))],
            ..Touched::default()
        };
        let changes = refresh(&mut index, &root, &touched, None);
        assert_eq!(
            changes,
            [
                VaultChange::Removed {
                    path: path("a/x.md"),
                    kind: EntryKind::Note
                },
                VaultChange::Renamed {
                    from: path("a"),
                    to: path("b"),
                    kind: EntryKind::Folder
                },
            ]
        );
        replay(&mut copy, &changes);
        assert_eq!(copy, mirror(&index));
    }

    #[test]
    fn renombrar_una_carpeta_con_otro_contenido_en_destino() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        fs::create_dir_all(root.join("a")).unwrap();
        fs::write(root.join("a/x.md"), "").unwrap();
        let mut index = scan(&root, None);
        fs::rename(root.join("a"), root.join("b")).unwrap();
        fs::write(root.join("b/nuevo.md"), "").unwrap();
        let path = |path: &str| VaultPath::new(path).unwrap();
        let touched = Touched {
            paths: [path("a"), path("b"), path("b/nuevo.md")].into(),
            renames: vec![(path("a"), path("b"))],
            ..Touched::default()
        };
        assert_eq!(
            refresh(&mut index, &root, &touched, None),
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
