//! De dónde viene el contenido de cada archivo al final de un lote.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::index::EntryKind;
use crate::order::subtree;
use crate::path::VaultPath;
use crate::refresh::Touched;

/// Para cada archivo tras el lote, la ruta que tenía antes su contenido, o
/// `None` si se escribió en el lote. Sale de repetir los renames en orden
/// sobre los archivos de antes, así que no depende de cómo se reporten.
pub(crate) struct Origins(BTreeMap<VaultPath, Option<VaultPath>>);

impl Origins {
    pub(crate) fn trace(before: &HashMap<VaultPath, EntryKind>, touched: &Touched) -> Self {
        let mut origins = Self(
            before
                .iter()
                .filter(|(_, kind)| **kind != EntryKind::Folder)
                .map(|(path, _)| (path.clone(), Some(path.clone())))
                .collect(),
        );
        // Escrito antes o después de moverse: se marca en los dos extremos.
        origins.write(&touched.written);
        for (from, to) in &touched.renames {
            origins.rename(from, to);
        }
        origins.write(&touched.written);
        origins
    }

    /// Si `path` tiene el mismo contenido que había en `old` antes del lote.
    pub(crate) fn kept(&self, path: &VaultPath, old: &VaultPath) -> bool {
        self.0
            .get(path)
            .is_some_and(|origin| origin.as_ref() == Some(old))
    }

    fn write(&mut self, paths: &BTreeSet<VaultPath>) {
        for path in paths {
            self.0.insert(path.clone(), None);
        }
    }

    fn rename(&mut self, from: &VaultPath, to: &VaultPath) {
        let moved: Vec<_> = subtree(&self.0, from)
            .filter_map(|(path, origin)| Some((path.rebase(from, to)?, origin.clone())))
            .collect();
        // Sin archivos que mover (falló, o era una carpeta vacía) el destino
        // conserva lo que tenía.
        if moved.is_empty() {
            return;
        }
        let gone: Vec<_> = subtree(&self.0, from)
            .chain(subtree(&self.0, to))
            .map(|(path, _)| path.clone())
            .collect();
        for path in gone {
            self.0.remove(&path);
        }
        self.0.extend(moved);
    }
}
