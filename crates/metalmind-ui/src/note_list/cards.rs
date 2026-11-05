//! La tarjeta de cada nota en la lista: su [`Summary`], que publica el vault, y
//! su fecha, lo único que la lista mira en el disco, en el fondo: todas al
//! empezar y después solo las que trae cada lote del watcher.

use std::collections::HashMap;
use std::path::Path;
use std::thread;
use std::time::SystemTime;

use metalmind_syntax::note_name;
use metalmind_vault::{EntryKind, Summary, VaultChange, VaultIndex, VaultPath};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Card<'a> {
    pub title: &'a str,
    pub excerpt: &'a str,
    /// El `mtime`: "modificada" (05 §4). `None` si no se pudo leer o no ha
    /// llegado.
    pub modified: Option<SystemTime>,
}

impl<'a> Card<'a> {
    /// La de `path`. Una nota sin título, o que no se pudo leer, se nombra por
    /// su archivo.
    #[must_use]
    pub fn new(
        path: &'a VaultPath,
        summary: Option<&'a Summary>,
        modified: Option<SystemTime>,
    ) -> Self {
        Self {
            title: summary
                .and_then(|summary| summary.title.as_deref())
                .unwrap_or_else(|| stem(path)),
            excerpt: summary.map_or("", |summary| summary.excerpt.as_str()),
            modified,
        }
    }
}

/// El nombre de la nota sin `.md`.
#[must_use]
pub fn stem(path: &VaultPath) -> &str {
    note_name::strip_extension(path.name()).unwrap_or(path.name())
}

/// El `mtime` de `paths`, con un hilo por núcleo. No abre ninguna nota.
#[must_use]
pub fn read_modified(root: &Path, paths: &[VaultPath]) -> Vec<(VaultPath, Option<SystemTime>)> {
    let workers = thread::available_parallelism().map_or(1, usize::from);
    let chunk = paths.len().div_ceil(workers).max(1);
    thread::scope(|scope| {
        let handles: Vec<_> = paths
            .chunks(chunk)
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .iter()
                        .map(|path| (path.clone(), modified(root, path)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| match handle.join() {
                Ok(dates) => dates,
                Err(panic) => std::panic::resume_unwind(panic),
            })
            .collect()
    })
}

fn modified(root: &Path, path: &VaultPath) -> Option<SystemTime> {
    let file = root.join(path.as_str());
    file.metadata().and_then(|meta| meta.modified()).ok()
}

/// La fecha de cada nota ya mirada; la que no se pudo leer guarda `None`.
#[derive(Clone, Debug, Default)]
pub struct ModifiedTimes(HashMap<VaultPath, Option<SystemTime>>);

impl ModifiedTimes {
    #[must_use]
    pub fn get(&self, path: &VaultPath) -> Option<SystemTime> {
        self.0.get(path).copied().flatten()
    }

    pub fn insert_all(&mut self, dates: impl IntoIterator<Item = (VaultPath, Option<SystemTime>)>) {
        self.0.extend(dates);
    }

    /// Quita lo borrado y mueve lo renombrado; devuelve las notas cuya fecha
    /// hay que volver a mirar. `index` es el del lote, ya con los cambios.
    pub fn apply(&mut self, changes: &[VaultChange], index: &VaultIndex) -> Vec<VaultPath> {
        let mut stale = Vec::new();
        for change in changes {
            match change {
                VaultChange::Created { path, .. } | VaultChange::Modified { path } => {
                    if index.kind(path) == Some(EntryKind::Note) {
                        stale.push(path.clone());
                    }
                }
                VaultChange::Removed { path, .. } => {
                    self.0.retain(|note, _| !note.starts_with(path));
                }
                VaultChange::Renamed { from, to, .. } => {
                    self.rename(from, to);
                    stale.extend(notes_under(index, to));
                }
            }
        }
        stale
    }

    /// La fecha de una carpeta renombrada se mueve con ella mientras se vuelve
    /// a mirar, para que la lista no se reordene.
    fn rename(&mut self, from: &VaultPath, to: &VaultPath) {
        let moved: Vec<VaultPath> = self
            .0
            .keys()
            .filter(|note| note.starts_with(from))
            .cloned()
            .collect();
        for old in moved {
            if let (Some(date), Some(new)) = (self.0.remove(&old), old.rebase(from, to)) {
                self.0.insert(new, date);
            }
        }
    }
}

/// `path` si es una nota, o las notas de la carpeta `path`.
fn notes_under<'i>(
    index: &'i VaultIndex,
    path: &'i VaultPath,
) -> impl Iterator<Item = VaultPath> + 'i {
    index
        .files()
        .filter(move |entry| entry.kind == EntryKind::Note && entry.path.starts_with(path))
        .map(|entry| entry.path.clone())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use pretty_assertions::assert_eq;

    use super::*;

    fn path(text: &str) -> VaultPath {
        VaultPath::new(text).unwrap()
    }

    #[test]
    fn sin_titulo_se_nombra_por_su_archivo() {
        let (con, sin, ilegible) = (path("con.md"), path("una idea.md"), path("no.md"));
        let titled = Summary {
            title: Some("El título".to_owned()),
            excerpt: "Texto.".to_owned(),
        };
        let untitled = Summary {
            title: None,
            excerpt: "Solo texto.".to_owned(),
        };
        let card = Card::new(&con, Some(&titled), None);
        assert_eq!((card.title, card.excerpt), ("El título", "Texto."));
        let card = Card::new(&sin, Some(&untitled), None);
        assert_eq!((card.title, card.excerpt), ("una idea", "Solo texto."));
        let card = Card::new(&ilegible, None, None);
        assert_eq!((card.title, card.excerpt), ("no", ""));
    }

    #[test]
    fn la_fecha_de_una_nota_que_no_esta_queda_vacia() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.md"), "").unwrap();
        let dates = read_modified(dir.path(), &[path("a.md"), path("no.md")]);
        let seen: Vec<(&str, bool)> = dates
            .iter()
            .map(|(path, date)| (path.as_str(), date.is_some()))
            .collect();
        assert_eq!(seen, [("a.md", true), ("no.md", false)]);
    }

    #[test]
    fn un_lote_del_watcher_dice_que_fechas_mirar() {
        let date = SystemTime::UNIX_EPOCH;
        let mut dates = ModifiedTimes::default();
        dates.insert_all([
            (path("a.md"), None),
            (path("d/b.md"), Some(date)),
            (path("borrada.md"), Some(date)),
        ]);
        // `e/c.md` llegó con la carpeta antes de tener fecha.
        let index = VaultIndex::from_entries(
            [
                ("a.md", EntryKind::Note),
                ("nueva.md", EntryKind::Note),
                ("img.png", EntryKind::Attachment),
                ("e", EntryKind::Folder),
                ("e/b.md", EntryKind::Note),
                ("e/c.md", EntryKind::Note),
                ("e/foto.png", EntryKind::Attachment),
            ]
            .map(|(name, kind)| (path(name), kind)),
        );
        let stale = dates.apply(
            &[
                VaultChange::Modified { path: path("a.md") },
                VaultChange::Modified {
                    path: path("img.png"),
                },
                VaultChange::Created {
                    path: path("nueva.md"),
                    kind: EntryKind::Note,
                },
                VaultChange::Created {
                    path: path("img.png"),
                    kind: EntryKind::Attachment,
                },
                VaultChange::Removed {
                    path: path("borrada.md"),
                    kind: EntryKind::Note,
                },
                VaultChange::Renamed {
                    from: path("d"),
                    to: path("e"),
                    kind: EntryKind::Folder,
                },
            ],
            &index,
        );
        let mut stale: Vec<&str> = stale.iter().map(VaultPath::as_str).collect();
        stale.sort_unstable();
        assert_eq!(stale, ["a.md", "e/b.md", "e/c.md", "nueva.md"]);
        assert_eq!(dates.get(&path("e/b.md")), Some(date));
        assert_eq!(dates.get(&path("d/b.md")), None);
        assert_eq!(dates.get(&path("borrada.md")), None);
    }
}
