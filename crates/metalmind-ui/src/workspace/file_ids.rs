//! Un [`FileId`] por archivo, para la jumplist y las marks `A`–`Z` del
//! motor: sobrevive a soltar su documento y sigue al archivo si se renombra
//! o se mueve, como el número de buffer de vim.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use metalmind_vim::FileId;

use crate::document::Document;

#[derive(Debug, Default)]
pub(super) struct FileIds {
    by_path: HashMap<PathBuf, FileId>,
    paths: HashMap<FileId, PathBuf>,
    next: u32,
}

impl FileIds {
    /// Uno nuevo, sin archivo, como el de un buffer `[No Name]`.
    pub fn fresh(&mut self) -> FileId {
        let id = FileId::new(self.next);
        self.next += 1;
        id
    }

    /// El del archivo de `document`, o uno nuevo si no tiene.
    pub fn of_document(&mut self, document: &Document) -> FileId {
        match document.resolved_path() {
            Some(path) => self.of_path(&path),
            None => self.fresh(),
        }
    }

    /// El de `path`, absoluta y sin enlaces; si no tenía, uno nuevo.
    pub fn of_path(&mut self, path: &Path) -> FileId {
        if let Some(&id) = self.by_path.get(path) {
            return id;
        }
        let id = self.fresh();
        self.adopt(id, path);
        id
    }

    /// `id`, que no tenía archivo, pasa a ser el de `path`, como un `[No
    /// Name]` al que `:w ruta` le da archivo. Si `path` ya era de otro (un
    /// archivo que se borró), devuelve ese, que se queda sin archivo.
    pub fn adopt(&mut self, id: FileId, path: &Path) -> Option<FileId> {
        let displaced = self
            .by_path
            .insert(path.to_owned(), id)
            .filter(|&old| old != id);
        if let Some(old) = displaced {
            self.paths.remove(&old);
        }
        self.paths.insert(id, path.to_owned());
        displaced
    }

    #[must_use]
    pub fn path(&self, id: FileId) -> Option<&Path> {
        self.paths.get(&id).map(PathBuf::as_path)
    }

    /// `from` (un archivo o una carpeta, con todo lo que contiene) pasó a
    /// llamarse `to`.
    pub fn rename(&mut self, from: &Path, to: &Path) {
        let moved: Vec<(PathBuf, FileId)> = self
            .by_path
            .iter()
            .filter(|(path, _)| path.starts_with(from))
            .map(|(path, &id)| (path.clone(), id))
            .collect();
        for (old, id) in moved {
            self.by_path.remove(&old);
            let rest = old.strip_prefix(from).unwrap_or(Path::new(""));
            let new = if rest.as_os_str().is_empty() {
                to.to_owned()
            } else {
                to.join(rest)
            };
            self.by_path.insert(new.clone(), id);
            self.paths.insert(id, new);
        }
    }

    /// Los de `path` y de lo que contiene.
    pub fn within(&self, path: &Path) -> Vec<FileId> {
        self.by_path
            .iter()
            .filter(|(own, _)| own.starts_with(path))
            .map(|(_, &id)| id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn un_archivo_tiene_siempre_el_mismo() {
        let mut ids = FileIds::default();
        let a = ids.of_path(Path::new("/v/a.md"));
        let b = ids.of_path(Path::new("/v/b.md"));
        assert_ne!(a, b);
        assert_eq!(ids.of_path(Path::new("/v/a.md")), a);
        assert_eq!(ids.path(b), Some(Path::new("/v/b.md")));
    }

    #[test]
    fn sigue_al_archivo_y_a_lo_de_una_carpeta_renombrada() {
        let mut ids = FileIds::default();
        let a = ids.of_path(Path::new("/v/d/a.md"));
        let b = ids.of_path(Path::new("/v/d/e/b.md"));
        let c = ids.of_path(Path::new("/v/dd.md"));
        ids.rename(Path::new("/v/d"), Path::new("/v/x"));
        assert_eq!(ids.path(a), Some(Path::new("/v/x/a.md")));
        assert_eq!(ids.path(b), Some(Path::new("/v/x/e/b.md")));
        assert_eq!(ids.path(c), Some(Path::new("/v/dd.md")));
        assert_eq!(ids.of_path(Path::new("/v/x/a.md")), a);
        assert_ne!(ids.of_path(Path::new("/v/d/a.md")), a);
    }

    #[test]
    fn un_buffer_sin_nombre_adopta_su_archivo() {
        let mut ids = FileIds::default();
        let unnamed = ids.fresh();
        assert_eq!(ids.path(unnamed), None);
        assert_eq!(ids.adopt(unnamed, Path::new("/v/n.md")), None);
        assert_eq!(ids.of_path(Path::new("/v/n.md")), unnamed);
    }

    /// La ruta era de un archivo que se borró: pasa a ser del buffer que se
    /// guardó ahí, y la vieja queda sin archivo.
    #[test]
    fn una_ruta_ocupada_pasa_al_buffer_que_la_adopta() {
        let mut ids = FileIds::default();
        let old = ids.of_path(Path::new("/v/n.md"));
        let unnamed = ids.fresh();
        assert_eq!(ids.adopt(unnamed, Path::new("/v/n.md")), Some(old));
        assert_eq!(ids.of_path(Path::new("/v/n.md")), unnamed);
        assert_eq!(ids.path(old), None);
    }
}
