//! La jumplist y las marks `A`–`Z` entre sesiones, como el shada de nvim:
//! en el estado del vault (06 §8), por ruta del vault, con la huella de
//! cada nota para seguir lo que se renombró con Metalmind cerrado (06 §1).
//!
//! El archivo es texto, una línea por dato y la ruta siempre al final:
//!
//! ```text
//! metalmind-saltos 1
//! nota <bytes> <blake3> <ruta>
//! salto <línea> <columna> <ruta>
//! mark <letra> <línea> <columna> <ruta>
//! ```

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::file::{self, FileError};
use crate::fingerprint::{Fingerprint, pair_by_content};
use crate::history::ContentHash;
use crate::index::{EntryKind, VaultIndex};
use crate::path::VaultPath;

const HEADER: &str = "metalmind-saltos 1";

/// Una posición en una nota: línea desde 0 y columna en bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedJump {
    pub path: VaultPath,
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SavedJumps {
    /// La jumplist, de la entrada más vieja a la más nueva.
    pub jumps: Vec<SavedJump>,
    /// Las marks puestas, con su letra de `A` a `Z`.
    pub marks: Vec<(char, SavedJump)>,
}

#[derive(Debug, thiserror::Error)]
pub enum SaveJumpsError {
    #[error("no se pudo crear {}: {source}", path.display())]
    CreateDir { path: PathBuf, source: io::Error },
    #[error(transparent)]
    Write(#[from] FileError),
}

impl SavedJumps {
    /// Lo escribe en `file`, de golpe, con la huella de cada nota de
    /// `root` que nombra. Una ruta con un salto de línea no cabe en el
    /// archivo y se omite.
    ///
    /// # Errors
    /// Si no se puede crear la carpeta de `file` o escribirlo.
    pub fn save(&self, root: &Path, file: &Path) -> Result<(), SaveJumpsError> {
        if let Some(dir) = file.parent() {
            fs::create_dir_all(dir).map_err(|source| SaveJumpsError::CreateDir {
                path: dir.to_owned(),
                source,
            })?;
        }
        let text = self.to_text(root);
        file::write_atomic(file, |out| out.write_all(text.as_bytes()))?;
        Ok(())
    }

    /// Lo guardado en `file` para el vault de `root`, cuyos archivos son
    /// los de `index`. Lo de una nota que ya no está pasa a la única con su
    /// mismo contenido (se renombró con Metalmind cerrado) o se descarta.
    /// Si no hay nada, es de otro formato o no se puede leer, no hay saltos.
    #[must_use]
    pub fn load(root: &Path, file: &Path, index: &VaultIndex) -> Self {
        let text = match file::read(file) {
            Ok(text) => text.unwrap_or_default(),
            Err(error) => {
                log::warn!("no se leyeron los saltos: {error}");
                return Self::default();
            }
        };
        let Some(parsed) = Parsed::new(&text) else {
            if !text.is_empty() {
                log::warn!("se ignora {}: no es de este formato", file.display());
            }
            return Self::default();
        };
        parsed.into_vault(root, index)
    }

    fn to_text(&self, root: &Path) -> String {
        let fits = |jump: &&SavedJump| !jump.path.as_str().contains('\n');
        let paths: BTreeSet<&VaultPath> = self
            .positions()
            .filter(fits)
            .map(|jump| &jump.path)
            .collect();
        let notes = paths.into_iter().filter_map(|path| {
            let Fingerprint { len, hash } = Fingerprint::of_note(root, path)?;
            let hash = blake3::Hash::from_bytes(*hash.as_bytes()).to_hex();
            Some(format!("nota {len} {hash} {}", path.as_str()))
        });
        let jumps = self
            .jumps
            .iter()
            .filter(fits)
            .map(|jump| format!("salto {}", position(jump)));
        let marks = self
            .marks
            .iter()
            .filter(|(_, jump)| fits(&jump))
            .map(|(letter, jump)| format!("mark {letter} {}", position(jump)));
        let mut lines: Vec<String> = [HEADER.to_owned()]
            .into_iter()
            .chain(notes)
            .chain(jumps)
            .chain(marks)
            .collect();
        lines.push(String::new());
        lines.join("\n")
    }

    fn positions(&self) -> impl Iterator<Item = &SavedJump> {
        self.jumps
            .iter()
            .chain(self.marks.iter().map(|(_, jump)| jump))
    }
}

fn position(jump: &SavedJump) -> String {
    format!("{} {} {}", jump.line, jump.column, jump.path.as_str())
}

/// El archivo leído, todavía con las rutas de cuando se guardó.
#[derive(Default)]
struct Parsed {
    fingerprints: HashMap<VaultPath, Fingerprint>,
    saved: SavedJumps,
}

impl Parsed {
    /// `None` si no empieza con [`HEADER`]. Una línea que no se entiende se
    /// ignora, y de una mark repetida vale la última.
    fn new(text: &str) -> Option<Self> {
        // Solo `\n` separa: en Linux un nombre puede terminar en `\r`.
        let mut lines = text.split('\n');
        if lines.next() != Some(HEADER) {
            return None;
        }
        let mut parsed = Self::default();
        let mut marks = [const { None }; 26];
        for line in lines {
            match line.split_once(' ') {
                Some(("nota", rest)) => {
                    if let Some((path, fingerprint)) = fingerprint(rest) {
                        parsed.fingerprints.insert(path, fingerprint);
                    }
                }
                Some(("salto", rest)) => parsed.saved.jumps.extend(saved_jump(rest)),
                Some(("mark", rest)) => {
                    if let Some((letter, rest)) = rest.split_once(' ')
                        && let Some(slot) = mark_slot(letter)
                        && let Some(jump) = saved_jump(rest)
                    {
                        marks[slot] = Some(jump);
                    }
                }
                _ => {}
            }
        }
        parsed.saved.marks = ('A'..='Z')
            .zip(marks)
            .filter_map(|(letter, jump)| Some((letter, jump?)))
            .collect();
        Some(parsed)
    }

    /// Con las rutas de ahora: las que siguen en `index` quedan, las que
    /// no pasan a la nota con su contenido y, si no hay, se descartan.
    fn into_vault(self, root: &Path, index: &VaultIndex) -> SavedJumps {
        let Self {
            fingerprints,
            saved,
        } = self;
        let exists = |path: &VaultPath| {
            index
                .kind(path)
                .is_some_and(|kind| kind != EntryKind::Folder)
        };
        let named: HashSet<VaultPath> = saved.positions().map(|jump| jump.path.clone()).collect();
        let lost = named
            .iter()
            .filter(|path| !exists(path))
            .filter_map(|path| Some((path.clone(), *fingerprints.get(path)?)));
        let candidates = index
            .files()
            .map(|entry| entry.path)
            .filter(|path| !named.contains(*path));
        let moved = pair_by_content(root, lost, candidates);
        let current = |jump: SavedJump| -> Option<SavedJump> {
            if exists(&jump.path) {
                return Some(jump);
            }
            let path = (*moved.get(&jump.path)?).clone();
            Some(SavedJump { path, ..jump })
        };
        SavedJumps {
            jumps: saved.jumps.into_iter().filter_map(current).collect(),
            marks: saved
                .marks
                .into_iter()
                .filter_map(|(letter, jump)| Some((letter, current(jump)?)))
                .collect(),
        }
    }
}

fn fingerprint(text: &str) -> Option<(VaultPath, Fingerprint)> {
    let mut parts = text.splitn(3, ' ');
    let len = parts.next()?.parse().ok()?;
    let hash = blake3::Hash::from_hex(parts.next()?).ok()?;
    let path = vault_path(parts.next()?)?;
    let hash = ContentHash::from_bytes(*hash.as_bytes());
    Some((path, Fingerprint { len, hash }))
}

fn saved_jump(text: &str) -> Option<SavedJump> {
    let mut parts = text.splitn(3, ' ');
    let line = parts.next()?.parse().ok()?;
    let column = parts.next()?.parse().ok()?;
    let path = vault_path(parts.next()?)?;
    Some(SavedJump { path, line, column })
}

fn vault_path(text: &str) -> Option<VaultPath> {
    VaultPath::new(text).ok().filter(|path| !path.is_root())
}

/// Del `0` al `25` para `A`–`Z`.
fn mark_slot(letter: &str) -> Option<usize> {
    let mut chars = letter.chars();
    let letter = chars.next().filter(char::is_ascii_uppercase)?;
    let letter = u8::try_from(letter).ok()?;
    chars.next().is_none().then(|| usize::from(letter - b'A'))
}

#[cfg(test)]
#[path = "saved_jumps_tests.rs"]
mod tests;
