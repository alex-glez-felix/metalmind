//! El índice de resúmenes: el [`Summary`] de cada nota, para que la lista de
//! notas los muestre sin leerlas.

use std::sync::Arc;

use imbl::HashMap;

use crate::index::VaultIndex;
use crate::note_batch::NoteBatch;
use crate::note_changes::{PerNoteIndex, Settle, apply_changes, notes_under};
use crate::path::VaultPath;
use crate::refresh::VaultChange;
use crate::summary::Summary;

/// Lo que cada nota muestra sin abrirla. Es un valor, como [`VaultIndex`]: un
/// clon comparte todo y cada cambio copia solo lo que toca.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SummaryIndex {
    by_note: HashMap<VaultPath, Arc<Summary>>,
}

impl SummaryIndex {
    /// El de `note`; una que no se pudo leer tiene uno vacío, y `None` es que
    /// no está en el vault.
    #[must_use]
    pub fn of(&self, note: &VaultPath) -> Option<&Summary> {
        self.by_note.get(note).map(AsRef::as_ref)
    }

    /// Con lo ya escaneado de cada nota.
    pub(crate) fn from_scans(scans: Vec<(VaultPath, Summary)>) -> Self {
        let by_note = scans
            .into_iter()
            .map(|(note, summary)| (note, Arc::new(summary)))
            .collect();
        Self { by_note }
    }

    /// Aplica los cambios de un lote, con `vault` ya actualizado.
    pub(crate) fn apply(
        &mut self,
        changes: &[VaultChange],
        vault: &VaultIndex,
        notes: &mut NoteBatch<'_>,
    ) {
        apply_changes(self, changes, vault, notes);
    }

    /// Vuelve a mirar las notas en `path` (y dentro, si es una carpeta) sin que
    /// el vault haya cambiado: un chmod puede volver legible o ilegible una
    /// nota. Dice si cambió algún resumen.
    pub(crate) fn reread(
        &mut self,
        path: &VaultPath,
        vault: &VaultIndex,
        notes: &mut NoteBatch<'_>,
    ) -> bool {
        let mut changed = false;
        for note in notes_under(vault, path) {
            let summary = &notes.scan(&note).summary;
            if self.of(&note) != Some(summary) {
                self.by_note.insert(note, Arc::new(summary.clone()));
                changed = true;
            }
        }
        changed
    }
}

impl PerNoteIndex for SummaryIndex {
    fn holds(&self, note: &VaultPath) -> bool {
        self.by_note.contains_key(note)
    }

    fn notes_within(&self, folder: &VaultPath) -> Vec<VaultPath> {
        self.by_note
            .keys()
            .filter(|note| note.starts_with(folder))
            .cloned()
            .collect()
    }

    fn forget(&mut self, note: &VaultPath) {
        self.by_note.remove(note);
    }

    fn moved(&mut self, old: &VaultPath, new: &VaultPath) -> Settle {
        if let Some(summary) = self.by_note.remove(old) {
            self.by_note.insert(new.clone(), summary);
        }
        Settle::IfMissing
    }

    fn rescan(&mut self, note: &VaultPath, notes: &mut NoteBatch<'_>) {
        let summary = notes.scan(note).summary.clone();
        self.by_note.insert(note.clone(), Arc::new(summary));
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::note_scan::NoteScan;
    use crate::refresh::{Touched, refresh};
    use crate::scan::scan;

    fn built(index: &VaultIndex, read: &dyn Fn(&VaultPath) -> Option<String>) -> SummaryIndex {
        let scans = index
            .files()
            .map(|entry| {
                let text = read(entry.path).unwrap_or_default();
                (entry.path.clone(), NoteScan::of(&text).summary)
            })
            .collect();
        SummaryIndex::from_scans(scans)
    }

    fn title(summaries: &SummaryIndex, note: &VaultPath) -> Option<String> {
        summaries.of(note)?.title.clone()
    }

    /// El resumen sigue a la nota: cambia al editarla y se va con ella al
    /// renombrarla, sin releerla.
    #[test]
    fn el_resumen_sigue_a_la_nota() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let read = |path: &VaultPath| fs::read_to_string(path.to_fs(&root)).ok();
        let (a, b) = (
            VaultPath::new("a.md").unwrap(),
            VaultPath::new("b.md").unwrap(),
        );
        fs::write(root.join("a.md"), "# Uno\nTexto.").unwrap();
        let mut index = scan(&root, None);
        let mut summaries = built(&index, &read);
        assert_eq!(summaries.of(&a).unwrap().excerpt, "Texto.");

        let mut batch = |touched: Touched, read: &dyn Fn(&VaultPath) -> Option<String>| {
            let changes = refresh(&mut index, &root, &touched, None);
            summaries.apply(&changes, &index, &mut NoteBatch::new(read));
            summaries.clone()
        };
        let written = |path: &VaultPath| Touched {
            paths: [path.clone()].into(),
            written: [path.clone()].into(),
            ..Touched::default()
        };
        fs::write(root.join("a.md"), "# Dos\n").unwrap();
        assert_eq!(title(&batch(written(&a), &read), &a).unwrap(), "Dos");

        fs::rename(root.join("a.md"), root.join("b.md")).unwrap();
        let moved = Touched {
            paths: [a.clone(), b.clone()].into(),
            renames: vec![(a.clone(), b.clone())],
            ..Touched::default()
        };
        let moved = batch(moved, &|_| panic!("un rename no relee la nota"));
        assert_eq!(moved.of(&a), None);
        assert_eq!(title(&moved, &b).unwrap(), "Dos");

        fs::remove_file(root.join("b.md")).unwrap();
        let removed = batch(written(&b), &read);
        assert_eq!(removed.of(&b), None);
    }

    #[test]
    fn releer_dice_si_cambio_algun_resumen() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        fs::create_dir(root.join("c")).unwrap();
        fs::write(root.join("c/a.md"), "# Uno\n").unwrap();
        let index = scan(&root, None);
        let read = |path: &VaultPath| fs::read_to_string(path.to_fs(&root)).ok();
        let mut summaries = built(&index, &read);
        let (folder, note) = (
            VaultPath::new("c").unwrap(),
            VaultPath::new("c/a.md").unwrap(),
        );
        assert!(!summaries.reread(&folder, &index, &mut NoteBatch::new(&read)));
        fs::write(root.join("c/a.md"), "# Dos\n").unwrap();
        assert!(summaries.reread(&folder, &index, &mut NoteBatch::new(&read)));
        assert_eq!(title(&summaries, &note).unwrap(), "Dos");
    }
}
