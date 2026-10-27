//! Lo que la lista sabe de cada nota sin abrirla: su [`Summary`] y cuándo
//! cambió. Se lee del disco en el fondo, todo al empezar y después solo lo
//! que trae cada lote del watcher.

use std::collections::HashMap;
use std::path::Path;
use std::thread;
use std::time::SystemTime;

use metalmind_syntax::note_name;
use metalmind_vault::{EntryKind, Summary, VaultChange, VaultPath};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Card {
    pub title: String,
    pub excerpt: String,
    /// El `mtime`: "modificada" (05 §4). `None` si no se pudo leer.
    pub modified: Option<SystemTime>,
}

impl Card {
    /// La de la nota `path` del vault en `root`. Una que no se puede leer
    /// sale solo con su nombre.
    #[must_use]
    pub fn read(root: &Path, path: &VaultPath) -> Self {
        let file = root.join(path.as_str());
        let modified = file.metadata().and_then(|meta| meta.modified()).ok();
        let summary = match metalmind_vault::read(&file) {
            Ok(Some(text)) => Summary::of(&text),
            Ok(None) | Err(_) => Summary::default(),
        };
        Self {
            title: summary.title.unwrap_or_else(|| stem(path).to_owned()),
            excerpt: summary.excerpt,
            modified,
        }
    }
}

/// El nombre de la nota sin `.md`.
#[must_use]
pub fn stem(path: &VaultPath) -> &str {
    note_name::strip_extension(path.name()).unwrap_or(path.name())
}

/// Las tarjetas de `paths`, leídas con un hilo por núcleo.
#[must_use]
pub fn read_all(root: &Path, paths: &[VaultPath]) -> Vec<(VaultPath, Card)> {
    let workers = thread::available_parallelism().map_or(1, usize::from);
    let chunk = paths.len().div_ceil(workers).max(1);
    thread::scope(|scope| {
        let handles: Vec<_> = paths
            .chunks(chunk)
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .iter()
                        .map(|path| (path.clone(), Card::read(root, path)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| match handle.join() {
                Ok(cards) => cards,
                Err(panic) => std::panic::resume_unwind(panic),
            })
            .collect()
    })
}

#[derive(Clone, Debug, Default)]
pub struct Cards(HashMap<VaultPath, Card>);

impl Cards {
    #[must_use]
    pub fn get(&self, path: &VaultPath) -> Option<&Card> {
        self.0.get(path)
    }

    pub fn insert_all(&mut self, cards: impl IntoIterator<Item = (VaultPath, Card)>) {
        self.0.extend(cards);
    }

    /// Quita lo borrado y mueve lo renombrado; devuelve las notas que hay que
    /// volver a leer.
    pub fn apply(&mut self, changes: &[VaultChange]) -> Vec<VaultPath> {
        let mut stale = Vec::new();
        for change in changes {
            match change {
                VaultChange::Created { path, kind } if *kind == EntryKind::Note => {
                    stale.push(path.clone());
                }
                VaultChange::Created { .. } => {}
                VaultChange::Modified { path } => stale.push(path.clone()),
                VaultChange::Removed { path, .. } => {
                    self.0.retain(|note, _| !note.starts_with(path));
                }
                VaultChange::Renamed { from, to, .. } => self.rename(from, to, &mut stale),
            }
        }
        stale
    }

    /// Una carpeta renombrada lleva sus notas; una nota cambia de nombre, que
    /// es su título si no tiene `# `: se vuelve a leer.
    fn rename(&mut self, from: &VaultPath, to: &VaultPath, stale: &mut Vec<VaultPath>) {
        let moved: Vec<VaultPath> = self
            .0
            .keys()
            .filter(|note| note.starts_with(from))
            .cloned()
            .collect();
        for old in moved {
            let (Some(card), Some(new)) = (self.0.remove(&old), old.rebase(from, to)) else {
                continue;
            };
            stale.push(new.clone());
            self.0.insert(new, card);
        }
    }
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
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("con.md"), "# El título\nTexto.").unwrap();
        fs::write(dir.path().join("una idea.md"), "Solo texto.").unwrap();
        let cards = read_all(
            dir.path(),
            &[path("con.md"), path("una idea.md"), path("no.md")],
        );
        let titles: Vec<(&str, &str, &str, bool)> = cards
            .iter()
            .map(|(path, card)| {
                let modified = card.modified.is_some();
                (
                    path.as_str(),
                    card.title.as_str(),
                    card.excerpt.as_str(),
                    modified,
                )
            })
            .collect();
        assert_eq!(
            titles,
            [
                ("con.md", "El título", "Texto.", true),
                ("una idea.md", "una idea", "Solo texto.", true),
                ("no.md", "no", "", false),
            ]
        );
    }

    #[test]
    fn un_lote_del_watcher_dice_que_releer() {
        let card = |title: &str| Card {
            title: title.to_owned(),
            excerpt: String::new(),
            modified: None,
        };
        let mut cards = Cards::default();
        cards.insert_all([
            (path("a.md"), card("a")),
            (path("d/b.md"), card("b")),
            (path("d/c.md"), card("c")),
            (path("borrada.md"), card("x")),
        ]);
        let stale = cards.apply(&[
            VaultChange::Modified { path: path("a.md") },
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
        ]);
        let mut stale: Vec<&str> = stale.iter().map(VaultPath::as_str).collect();
        stale.sort_unstable();
        assert_eq!(stale, ["a.md", "e/b.md", "e/c.md", "nueva.md"]);
        assert_eq!(
            cards.get(&path("e/b.md")).map(|card| card.title.as_str()),
            Some("b")
        );
        assert_eq!(cards.get(&path("d/b.md")), None);
        assert_eq!(cards.get(&path("borrada.md")), None);
    }
}
