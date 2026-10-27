//! Aplicar los cambios del watcher da el mismo árbol que armarlo de cero
//! con el índice final, y lo abierto y lo seleccionado siguen a cada
//! archivo a través de sus renombres.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use metalmind_vault::{EntryKind, VaultChange, VaultIndex, VaultPath};
use proptest::prelude::*;

use super::FileTree;
use crate::panel::Outline as _;

/// Una operación sobre el disco; los índices eligen entre lo que exista.
#[derive(Clone, Debug)]
enum Op {
    Create {
        parent: usize,
        name: usize,
        folder: bool,
    },
    Remove {
        pick: usize,
    },
    Rename {
        pick: usize,
        parent: usize,
        name: usize,
    },
}

const NAMES: &[&str] = &["a", "b.md", "C", "d.md", "e.png"];

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (any::<usize>(), 0..NAMES.len(), any::<bool>()).prop_map(|(parent, name, folder)| {
            Op::Create {
                parent,
                name,
                folder,
            }
        }),
        any::<usize>().prop_map(|pick| Op::Remove { pick }),
        (any::<usize>(), any::<usize>(), 0..NAMES.len())
            .prop_map(|(pick, parent, name)| Op::Rename { pick, parent, name }),
    ]
}

/// El disco: cada ruta con su tipo y una identidad que no cambia al
/// renombrar.
#[derive(Default)]
struct Disk {
    entries: BTreeMap<VaultPath, (EntryKind, u32)>,
    next_id: u32,
}

impl Disk {
    fn index(&self) -> Arc<VaultIndex> {
        Arc::new(VaultIndex::from_entries(
            self.entries
                .iter()
                .map(|(path, &(kind, _))| (path.clone(), kind)),
        ))
    }

    fn folders(&self) -> Vec<VaultPath> {
        std::iter::once(VaultPath::root())
            .chain(
                self.entries
                    .iter()
                    .filter(|(_, (kind, _))| *kind == EntryKind::Folder)
                    .map(|(path, _)| path.clone()),
            )
            .collect()
    }

    fn pick(&self, pick: usize) -> Option<VaultPath> {
        let len = self.entries.len();
        (len > 0).then(|| self.entries.keys().nth(pick % len).cloned())?
    }

    fn path_of(&self, id: u32) -> Option<&VaultPath> {
        self.entries
            .iter()
            .find(|(_, (_, other))| *other == id)
            .map(|(path, _)| path)
    }

    fn id_of(&self, path: &VaultPath) -> Option<u32> {
        self.entries.get(path).map(|&(_, id)| id)
    }

    /// Aplica `op` y devuelve los cambios que publicaría el watcher, en un
    /// orden que se puede aplicar uno a uno.
    fn apply(&mut self, op: &Op) -> Vec<VaultChange> {
        match *op {
            Op::Create {
                parent,
                name,
                folder,
            } => {
                let folders = self.folders();
                let parent = &folders[parent % folders.len()];
                let path = parent.join(NAMES[name]).unwrap();
                if self.entries.contains_key(&path) {
                    return Vec::new();
                }
                let kind = match (folder, NAMES[name]) {
                    (true, _) => EntryKind::Folder,
                    (false, "b.md" | "d.md") => EntryKind::Note,
                    (false, _) => EntryKind::Attachment,
                };
                self.entries.insert(path.clone(), (kind, self.next_id));
                self.next_id += 1;
                vec![VaultChange::Created { path, kind }]
            }
            Op::Remove { pick } => {
                let Some(removed) = self.pick(pick) else {
                    return Vec::new();
                };
                let mut gone: Vec<(VaultPath, EntryKind)> = self
                    .entries
                    .iter()
                    .filter(|(path, _)| path.starts_with(&removed))
                    .map(|(path, &(kind, _))| (path.clone(), kind))
                    .collect();
                gone.sort_by_key(|(path, _)| std::cmp::Reverse(path.depth()));
                for (path, _) in &gone {
                    self.entries.remove(path);
                }
                gone.into_iter()
                    .map(|(path, kind)| VaultChange::Removed { path, kind })
                    .collect()
            }
            Op::Rename { pick, parent, name } => {
                let Some(from) = self.pick(pick) else {
                    return Vec::new();
                };
                let folders = self.folders();
                let parent = &folders[parent % folders.len()];
                let to = parent.join(NAMES[name]).unwrap();
                if self.entries.contains_key(&to) || to.starts_with(&from) {
                    return Vec::new();
                }
                let moved: Vec<(VaultPath, (EntryKind, u32))> = self
                    .entries
                    .iter()
                    .filter(|(path, _)| path.starts_with(&from))
                    .map(|(path, &entry)| (path.clone(), entry))
                    .collect();
                for (path, entry) in moved {
                    self.entries.remove(&path);
                    self.entries.insert(path.rebase(&from, &to).unwrap(), entry);
                }
                let kind = self.entries[&to].0;
                vec![VaultChange::Renamed { from, to, kind }]
            }
        }
    }
}

proptest! {
    #[test]
    fn aplicar_cambios_da_el_arbol_de_cero(
        setup in prop::collection::vec(op(), 0..25),
        open in prop::collection::vec(any::<bool>(), 25),
        selected in any::<usize>(),
        batches in prop::collection::vec(prop::collection::vec(op(), 1..5), 1..10),
    ) {
        let mut disk = Disk::default();
        for op in &setup {
            disk.apply(op);
        }
        let folders = disk.folders();
        let expanded: Vec<VaultPath> = folders
            .iter()
            .skip(1)
            .zip(&open)
            .filter(|&(_, &open)| open)
            .map(|(path, _)| path.clone())
            .collect();
        let expanded_ids: BTreeSet<u32> =
            expanded.iter().filter_map(|path| disk.id_of(path)).collect();
        let mut tree = FileTree::with_expanded(disk.index(), expanded);
        if let Some(row) = tree.rows().len().checked_sub(1).map(|last| selected % (last + 1)) {
            tree.select_row(row);
        }
        let mut selected_id = tree.selected().and_then(|path| disk.id_of(path));

        for batch in &batches {
            let changes: Vec<VaultChange> = batch.iter().flat_map(|op| disk.apply(op)).collect();
            tree.apply(&changes, disk.index(), None);
            check_selection(&tree, &disk, selected_id)?;
            // Si cayó en otra fila, desde aquí se sigue a la nueva.
            selected_id = tree.selected().and_then(|path| disk.id_of(path));
        }

        let still_open: BTreeSet<VaultPath> = expanded_ids
            .iter()
            .filter_map(|&id| disk.path_of(id).cloned())
            .collect();
        prop_assert_eq!(tree.expanded().cloned().collect::<BTreeSet<_>>(), still_open.clone());
        let fresh = FileTree::with_expanded(disk.index(), still_open);
        prop_assert_eq!(tree.rows(), fresh.rows());
    }
}

/// Lo seleccionado sigue a su archivo mientras se vea; si no se ve, queda
/// una fila que sí se ve, y nada solo con el árbol vacío.
fn check_selection(
    tree: &FileTree,
    disk: &Disk,
    selected_id: Option<u32>,
) -> Result<(), TestCaseError> {
    let visible = |path: &VaultPath| tree.rows().iter().any(|row| &row.path == path);
    match selected_id
        .and_then(|id| disk.path_of(id))
        .filter(|path| visible(path))
    {
        Some(path) => prop_assert_eq!(tree.selected(), Some(path)),
        None if tree.rows().is_empty() => prop_assert_eq!(tree.selected(), None),
        None => prop_assert!(tree.selected().is_some_and(visible)),
    }
    prop_assert_eq!(
        tree.selected_row().map(|row| &tree.rows()[row].path),
        tree.selected()
    );
    Ok(())
}
