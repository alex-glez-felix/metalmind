//! Los documentos abiertos, cada uno con su buffer, su historial y su estado
//! de guardado. Un archivo se abre una sola vez: dos paneles sobre la misma
//! ruta comparten documento.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use crate::document::Document;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocumentId(u32);

impl fmt::Display for DocumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Default)]
pub struct Documents {
    open: BTreeMap<DocumentId, Document>,
    next_id: u32,
}

impl Documents {
    pub fn insert(&mut self, document: Document) -> DocumentId {
        let id = DocumentId(self.next_id);
        self.next_id += 1;
        self.open.insert(id, document);
        id
    }

    pub fn remove(&mut self, id: DocumentId) -> Option<Document> {
        self.open.remove(&id)
    }

    #[must_use]
    pub fn get(&self, id: DocumentId) -> Option<&Document> {
        self.open.get(&id)
    }

    pub fn get_mut(&mut self, id: DocumentId) -> Option<&mut Document> {
        self.open.get_mut(&id)
    }

    /// El documento abierto de ese archivo, se haya escrito como se haya
    /// escrito la ruta.
    #[must_use]
    pub fn find(&self, path: &Path) -> Option<DocumentId> {
        self.open
            .iter()
            .find(|(_, document)| document.is_file(path))
            .map(|(&id, _)| id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (DocumentId, &Document)> {
        self.open.iter().map(|(&id, document)| (id, document))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn encuentra_el_archivo_por_cualquier_ruta() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("notas")).unwrap();
        let path = dir.path().join("notas").join("a.md");
        fs::write(&path, "a\n").unwrap();
        let mut documents = Documents::default();
        let a = documents.insert(Document::open(&path).unwrap());
        let b = documents.insert(Document::open(&dir.path().join("b.md")).unwrap());
        assert_ne!(a, b);
        let roundabout = dir
            .path()
            .join("notas")
            .join("..")
            .join("notas")
            .join("a.md");
        assert_eq!(documents.find(&roundabout), Some(a));
        assert_eq!(documents.find(&dir.path().join("b.md")), Some(b));
        assert_eq!(documents.find(&dir.path().join("c.md")), None);
    }

    #[test]
    fn quitar_libera_el_documento() {
        let mut documents = Documents::default();
        let a = documents.insert(Document::new(Path::new("a.md"), "a"));
        assert!(documents.remove(a).is_some());
        assert_eq!(documents.iter().count(), 0);
        assert!(documents.get(a).is_none());
        let b = documents.insert(Document::new(Path::new("a.md"), "a"));
        assert_ne!(a, b);
    }
}
