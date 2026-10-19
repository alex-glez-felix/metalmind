//! Los documentos abiertos, cada uno con su buffer, su historial y su estado
//! de guardado. Un archivo se abre una sola vez: dos paneles sobre la misma
//! ruta comparten documento.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use metalmind_vim::{FileId, Rows};

use crate::document::Document;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocumentId(u32);

impl fmt::Display for DocumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug)]
pub struct Documents {
    open: BTreeMap<DocumentId, Document>,
    /// El archivo de cada uno para el motor vim.
    files: BTreeMap<DocumentId, FileId>,
    next_id: u32,
    /// Lo que mide cada línea en todos, en píxeles.
    rows: Rows,
}

impl Default for Documents {
    fn default() -> Self {
        Self {
            open: BTreeMap::new(),
            files: BTreeMap::new(),
            next_id: 0,
            rows: Rows::uniform(1),
        }
    }
}

impl Documents {
    pub fn insert(&mut self, mut document: Document, file: FileId) -> DocumentId {
        document.set_rows(&self.rows);
        // Desde 1, como los números de los buffers de vim.
        self.next_id += 1;
        let id = DocumentId(self.next_id);
        self.open.insert(id, document);
        self.files.insert(id, file);
        id
    }

    #[must_use]
    pub fn rows(&self) -> &Rows {
        &self.rows
    }

    /// Lo que mide cada línea, en todos. Devuelve si cambió.
    pub fn set_rows(&mut self, rows: Rows) -> bool {
        if rows == self.rows {
            return false;
        }
        for document in self.open.values_mut() {
            document.set_rows(&rows);
        }
        self.rows = rows;
        true
    }

    pub fn remove(&mut self, id: DocumentId) -> Option<Document> {
        self.files.remove(&id);
        self.open.remove(&id)
    }

    #[must_use]
    pub fn file(&self, id: DocumentId) -> Option<FileId> {
        self.files.get(&id).copied()
    }

    /// El documento abierto de `file`.
    #[must_use]
    pub fn of_file(&self, file: FileId) -> Option<DocumentId> {
        self.files
            .iter()
            .find(|&(_, &own)| own == file)
            .map(|(&id, _)| id)
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
        let a = documents.insert(Document::open(&path).unwrap(), FileId::new(0));
        let b = documents.insert(
            Document::open(&dir.path().join("b.md")).unwrap(),
            FileId::new(1),
        );
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
        let a = documents.insert(Document::new(Path::new("a.md"), "a"), FileId::new(0));
        assert!(documents.remove(a).is_some());
        assert_eq!(documents.iter().count(), 0);
        assert!(documents.get(a).is_none());
        let b = documents.insert(Document::new(Path::new("a.md"), "a"), FileId::new(0));
        assert_ne!(a, b);
    }
}
