//! Las notas que mira un lote del watcher: cada una se lee y se escanea una
//! sola vez, y todos los que la miran en el lote ven el mismo contenido.

use std::collections::HashMap;

use crate::note_scan::NoteScan;
use crate::path::VaultPath;

type Read<'r> = dyn Fn(&VaultPath) -> Option<String> + 'r;

/// Lo que un índice recibe para ponerse al día. Vive un lote: lo siguiente que
/// cambie llega en otro, con lo que tenga el disco entonces.
pub struct NoteBatch<'r> {
    read: &'r Read<'r>,
    notes: HashMap<VaultPath, BatchNote>,
}

/// Una nota que no se pudo leer no tiene texto y queda sin nada escaneado.
struct BatchNote {
    text: Option<String>,
    scan: NoteScan,
}

impl<'r> NoteBatch<'r> {
    /// `read` da el contenido de una nota, o `None` si no se pudo leer.
    #[must_use]
    pub fn new(read: &'r Read<'r>) -> Self {
        Self {
            read,
            notes: HashMap::new(),
        }
    }

    pub(crate) fn scan(&mut self, path: &VaultPath) -> &NoteScan {
        &self.note(path).scan
    }

    pub(crate) fn text(&mut self, path: &VaultPath) -> Option<&str> {
        self.note(path).text.as_deref()
    }

    /// El texto de las `paths` ya leídas, una vez por nota: aquí no se lee nada.
    pub(crate) fn into_texts(mut self, paths: &[VaultPath]) -> Vec<(VaultPath, String)> {
        paths
            .iter()
            .filter_map(|path| Some((path.clone(), self.notes.remove(path)?.text?)))
            .collect()
    }

    fn note(&mut self, path: &VaultPath) -> &BatchNote {
        let read = self.read;
        self.notes.entry(path.clone()).or_insert_with(|| {
            let text = read(path);
            let scan = text
                .as_deref()
                .map(|text| NoteScan::of(path, text))
                .unwrap_or_default();
            BatchNote { text, scan }
        })
    }
}

impl std::fmt::Debug for NoteBatch<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NoteBatch")
            .field("notes", &self.notes.len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    #[test]
    fn una_nota_se_lee_una_vez_aunque_la_pidan_varios() {
        let reads = Cell::new(0);
        let read = |_: &VaultPath| {
            reads.set(reads.get() + 1);
            Some("[[b]] #rust".to_owned())
        };
        let mut batch = NoteBatch::new(&read);
        let note = VaultPath::new("a.md").unwrap();
        assert_eq!(batch.scan(&note).links.len(), 1);
        assert_eq!(batch.scan(&note).tags.occurrences.len(), 1);
        assert_eq!(batch.text(&note), Some("[[b]] #rust"));
        let texts = batch.into_texts(std::slice::from_ref(&note));
        assert_eq!(texts, [(note, "[[b]] #rust".to_owned())]);
        assert_eq!(reads.get(), 1);
    }

    #[test]
    fn los_textos_no_leen_ni_repiten() {
        let reads = Cell::new(0);
        let read = |_: &VaultPath| {
            reads.set(reads.get() + 1);
            Some("texto".to_owned())
        };
        let mut batch = NoteBatch::new(&read);
        let (a, b) = (
            VaultPath::new("a.md").unwrap(),
            VaultPath::new("b.md").unwrap(),
        );
        batch.text(&a);
        let texts = batch.into_texts(&[a.clone(), b, a.clone()]);
        assert_eq!(texts, [(a, "texto".to_owned())]);
        assert_eq!(reads.get(), 1);
    }

    #[test]
    fn la_que_no_se_puede_leer_queda_sin_nada() {
        let reads = Cell::new(0);
        let read = |_: &VaultPath| {
            reads.set(reads.get() + 1);
            None
        };
        let mut batch = NoteBatch::new(&read);
        let note = VaultPath::new("a.md").unwrap();
        assert_eq!(batch.scan(&note), &NoteScan::default());
        assert_eq!(batch.text(&note), None);
        assert_eq!(batch.into_texts(std::slice::from_ref(&note)), []);
        assert_eq!(reads.get(), 1);
    }
}
