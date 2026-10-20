//! `space f`: el finder de notas, un picker sobre las rutas del vault. Con
//! el query vacío, las notas abiertas hace poco van primero.

mod recents;
mod session;
mod writer;

use std::collections::HashSet;
use std::sync::Arc;

use metalmind_vault::{EntryKind, VaultIndex, VaultPath};

pub use self::recents::Recents;
pub use self::session::{Prepared, SessionError, VaultChanged, VaultSession, WatchLost};
pub(crate) use self::session::{canonical, relative_to};
use crate::picker::PickerItem;

/// Una nota del finder, por su ruta en el vault.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note(pub VaultPath);

impl AsRef<str> for Note {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl PickerItem for Note {}

/// Lo que lista el finder: las recientes que siguen en el vault, de la
/// última a la primera, y después las demás notas por ruta.
#[must_use]
pub fn notes(index: &VaultIndex, recents: &Recents) -> Arc<[Note]> {
    let is_note = |path: &VaultPath| index.kind(path) == Some(EntryKind::Note);
    let recent: Vec<&VaultPath> = recents
        .paths()
        .iter()
        .filter(|path| is_note(path))
        .collect();
    let seen: HashSet<&VaultPath> = recent.iter().copied().collect();
    let mut rest: Vec<&VaultPath> = index
        .files()
        .filter(|entry| entry.kind == EntryKind::Note && !seen.contains(entry.path))
        .map(|entry| entry.path)
        .collect();
    rest.sort_unstable();
    recent
        .into_iter()
        .chain(rest)
        .map(|path| Note(path.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use metalmind_vault::Vault;
    use pretty_assertions::assert_eq;

    use super::*;

    fn names(notes: &[Note]) -> Vec<&str> {
        notes.iter().map(AsRef::as_ref).collect()
    }

    #[test]
    fn las_recientes_van_primero_y_solo_hay_notas() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["b.md", "a.md", "d/c.md", "img.png"] {
            let path = dir.path().join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "").unwrap();
        }
        let (vault, _updates) = Vault::open(dir.path()).unwrap();
        let index = vault.snapshot();
        assert_eq!(
            names(&notes(&index, &Recents::default())),
            ["a.md", "b.md", "d/c.md"]
        );
        let recents = Recents::parse("d/c.md\nborrada.md\nimg.png\nb.md\n");
        assert_eq!(names(&notes(&index, &recents)), ["d/c.md", "b.md", "a.md"]);
    }
}
