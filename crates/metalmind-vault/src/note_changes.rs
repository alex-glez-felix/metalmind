//! Cómo un índice con una entrada por nota sigue los cambios de un lote: qué
//! se olvida, qué se muda sin releerse y qué se escanea al final, cuando el
//! lote ya se sabe entero.

use std::collections::BTreeMap;

use crate::index::{EntryKind, VaultIndex};
use crate::note_batch::NoteBatch;
use crate::path::VaultPath;
use crate::refresh::VaultChange;

/// Cuánto hay que mirar de nuevo una ruta al final del lote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Settle {
    /// Llegó por un rename: solo se escanea si no estaba en el índice.
    IfMissing,
    /// Su contenido cambió, o el índice pidió releerla al mudarla.
    Rescan,
}

/// Un índice con una entrada por nota, sin importar qué guarde de ella.
pub(crate) trait PerNoteIndex {
    fn holds(&self, note: &VaultPath) -> bool;
    /// Las notas indexadas dentro de `folder`.
    fn notes_within(&self, folder: &VaultPath) -> Vec<VaultPath>;
    fn forget(&mut self, note: &VaultPath);
    /// Muda lo de `old` a `new` y dice cuánto mirar `new` al final del lote.
    fn moved(&mut self, old: &VaultPath, new: &VaultPath) -> Settle;
    fn rescan(&mut self, note: &VaultPath, notes: &mut NoteBatch<'_>);
}

/// Lleva `index` a lo que dicen los cambios de un lote, con `vault` ya
/// actualizado. Una nota renombrada conserva lo suyo sin releerse.
pub(crate) fn apply_changes(
    index: &mut impl PerNoteIndex,
    changes: &[VaultChange],
    vault: &VaultIndex,
    notes: &mut NoteBatch<'_>,
) {
    let mut settle: BTreeMap<VaultPath, Settle> = BTreeMap::new();
    let mark = |settle: &mut BTreeMap<VaultPath, Settle>, path: VaultPath, how: Settle| {
        let entry = settle.entry(path).or_insert(how);
        *entry = (*entry).max(how);
    };
    for change in changes {
        match change {
            VaultChange::Created { path, .. } | VaultChange::Modified { path } => {
                mark(&mut settle, path.clone(), Settle::Rescan);
            }
            VaultChange::Removed { path, kind } => {
                for note in indexed_under(index, path, *kind) {
                    index.forget(&note);
                }
            }
            VaultChange::Renamed { from, to, kind } => {
                for old in indexed_under(index, from, *kind) {
                    let Some(new) = old.rebase(from, to) else {
                        continue;
                    };
                    let how = index.moved(&old, &new);
                    mark(&mut settle, new, how);
                }
                for note in notes_under(vault, to) {
                    mark(&mut settle, note, Settle::IfMissing);
                }
            }
        }
    }
    for (path, how) in settle {
        if vault.kind(&path) != Some(EntryKind::Note) {
            index.forget(&path);
        } else if how == Settle::Rescan || !index.holds(&path) {
            index.rescan(&path, notes);
        }
    }
}

/// Las notas indexadas que son `path` o, si es carpeta, están dentro de él.
fn indexed_under(index: &impl PerNoteIndex, path: &VaultPath, kind: EntryKind) -> Vec<VaultPath> {
    match kind {
        EntryKind::Note => vec![path.clone()],
        EntryKind::Board | EntryKind::Attachment => Vec::new(),
        EntryKind::Folder => index.notes_within(path),
    }
}

/// Las notas de `vault` que son `path` o están dentro de él.
pub(crate) fn notes_under(vault: &VaultIndex, path: &VaultPath) -> Vec<VaultPath> {
    vault
        .subtree(path)
        .into_iter()
        .filter(|(_, kind)| *kind == EntryKind::Note)
        .map(|(note, _)| note)
        .collect()
}
