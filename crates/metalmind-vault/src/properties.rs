//! El índice de propiedades: el frontmatter de cada nota como clave y valor,
//! de solo lectura, para las consultas por cualquier campo (03 §4).

use std::sync::Arc;

use imbl::HashMap;
use metalmind_syntax::Property;

use crate::index::VaultIndex;
use crate::note_batch::NoteBatch;
use crate::note_changes::{PerNoteIndex, Settle, apply_changes, notes_under};
use crate::path::VaultPath;
use crate::refresh::VaultChange;

/// Lo que el frontmatter de cada nota dice. Es un valor, como
/// [`VaultIndex`]: un clon comparte todo y cada cambio copia solo lo que toca.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PropertyIndex {
    by_note: HashMap<VaultPath, Arc<[Property]>>,
}

impl PropertyIndex {
    /// Cada clave escalar de primer nivel del frontmatter de `note` con su
    /// valor, en orden; vacío si no tiene, no se pudo leer o no es un mapa.
    #[must_use]
    pub fn of(&self, note: &VaultPath) -> &[Property] {
        self.by_note.get(note).map_or(&[], AsRef::as_ref)
    }

    /// Con lo ya escaneado de cada nota.
    pub(crate) fn from_scans(scans: Vec<(VaultPath, Vec<Property>)>) -> Self {
        let by_note = scans
            .into_iter()
            .map(|(note, properties)| (note, Arc::from(properties)))
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
    /// nota. Dice si cambió alguna propiedad.
    pub(crate) fn reread(
        &mut self,
        path: &VaultPath,
        vault: &VaultIndex,
        notes: &mut NoteBatch<'_>,
    ) -> bool {
        let mut changed = false;
        for note in notes_under(vault, path) {
            let properties = notes.scan(&note).properties.as_slice();
            if self.of(&note) != properties {
                self.by_note.insert(note, Arc::from(properties));
                changed = true;
            }
        }
        changed
    }
}

impl PerNoteIndex for PropertyIndex {
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
        if let Some(properties) = self.by_note.remove(old) {
            self.by_note.insert(new.clone(), properties);
        }
        Settle::IfMissing
    }

    fn rescan(&mut self, note: &VaultPath, notes: &mut NoteBatch<'_>) {
        let properties = notes.scan(note).properties.as_slice();
        self.by_note.insert(note.clone(), Arc::from(properties));
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use metalmind_syntax::PropertyValue;
    use tempfile::TempDir;

    use super::*;
    use crate::note_scan::NoteScan;
    use crate::refresh::{Touched, refresh};
    use crate::scan::scan;

    fn title(text: &str) -> Vec<Property> {
        vec![Property {
            key: "título".to_owned(),
            value: PropertyValue::Text(text.to_owned()),
        }]
    }

    fn built(index: &VaultIndex, read: &dyn Fn(&VaultPath) -> Option<String>) -> PropertyIndex {
        let scans = index
            .files()
            .map(|entry| {
                let text = read(entry.path).unwrap_or_default();
                (entry.path.clone(), NoteScan::of(&text).properties)
            })
            .collect();
        PropertyIndex::from_scans(scans)
    }

    /// Las propiedades siguen a la nota: cambian al editarla y se van con ella
    /// al renombrarla, sin releerla.
    #[test]
    fn las_propiedades_siguen_a_la_nota() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let read = |path: &VaultPath| fs::read_to_string(path.to_fs(&root)).ok();
        let (a, b) = (
            VaultPath::new("a.md").unwrap(),
            VaultPath::new("b.md").unwrap(),
        );
        fs::write(root.join("a.md"), "---\ntítulo: Uno\n---\n").unwrap();
        let mut index = scan(&root, None);
        let mut properties = built(&index, &read);
        assert_eq!(properties.of(&a), title("Uno"));

        let mut batch = |touched: Touched, read: &dyn Fn(&VaultPath) -> Option<String>| {
            let changes = refresh(&mut index, &root, &touched, None);
            properties.apply(&changes, &index, &mut NoteBatch::new(read));
            properties.clone()
        };
        let written = |path: &VaultPath| Touched {
            paths: [path.clone()].into(),
            written: [path.clone()].into(),
            ..Touched::default()
        };
        fs::write(root.join("a.md"), "---\ntítulo: Dos\n---\n").unwrap();
        assert_eq!(batch(written(&a), &read).of(&a), title("Dos"));

        fs::rename(root.join("a.md"), root.join("b.md")).unwrap();
        let moved = Touched {
            paths: [a.clone(), b.clone()].into(),
            renames: vec![(a.clone(), b.clone())],
            ..Touched::default()
        };
        let moved = batch(moved, &|_| panic!("un rename no relee la nota"));
        assert_eq!(moved.of(&a), []);
        assert_eq!(moved.of(&b), title("Dos"));

        fs::write(root.join("b.md"), "sin frontmatter").unwrap();
        assert_eq!(batch(written(&b), &read).of(&b), []);
    }

    #[test]
    fn releer_dice_si_cambio_alguna_propiedad() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        fs::create_dir(root.join("c")).unwrap();
        fs::write(root.join("c/a.md"), "---\ntítulo: Uno\n---\n").unwrap();
        let index = scan(&root, None);
        let read = |path: &VaultPath| fs::read_to_string(path.to_fs(&root)).ok();
        let mut properties = built(&index, &read);
        let (folder, note) = (
            VaultPath::new("c").unwrap(),
            VaultPath::new("c/a.md").unwrap(),
        );
        assert!(!properties.reread(&folder, &index, &mut NoteBatch::new(&read)));
        fs::write(root.join("c/a.md"), "---\ntítulo: Dos\n---\n").unwrap();
        assert!(properties.reread(&folder, &index, &mut NoteBatch::new(&read)));
        assert_eq!(properties.of(&note), title("Dos"));
    }
}
