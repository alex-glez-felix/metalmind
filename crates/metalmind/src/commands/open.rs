//! `metalmind open`: de una ruta del vault a la nota que abre la ventana.

use std::fs;
use std::path::{Path, PathBuf};

use metalmind_vault::{EntryKind, VaultPath, VaultRoot};
use serde::Serialize;

use super::{CliError, print, root_text, vault_root};
use crate::cli::OpenArgs;
use crate::target::Target;

/// Una nota del vault, lista para abrir.
#[derive(Debug, Serialize)]
pub struct Note {
    vault: String,
    path: VaultPath,
    #[serde(skip)]
    root: PathBuf,
}

impl Note {
    /// El vault con esta nota abierta.
    pub fn target(&self) -> Target {
        Target {
            vault: self.root.clone(),
            file: Some(self.root.join(self.path.as_str())),
        }
    }

    /// Con `--json`, dice qué se va a abrir; sin él, nada.
    ///
    /// # Errors
    /// Si no se puede escribir en stdout.
    pub fn announce(&self, json: bool) -> Result<(), CliError> {
        print(json, self, |_| Ok(()))
    }
}

/// La nota de `args`, sin abrir todavía el vault entero: eso lo hace la ventana.
///
/// # Errors
/// Si no hay vault o `args.path` no es una nota que exista en él.
pub fn open(args: &OpenArgs) -> Result<Note, CliError> {
    let root = vault_root(&args.common)?;
    let path = note_in(&root, &args.path)?;
    Ok(Note {
        vault: root_text(root.path()).into_owned(),
        root: root.path().to_owned(),
        path,
    })
}

/// `path` como nota de `root`: relativa a la raíz o absoluta dentro de ella,
/// un `.md` que existe y al que no se llega por un enlace.
fn note_in(root: &VaultRoot, path: &Path) -> Result<VaultPath, CliError> {
    let not_a_note = || CliError::NotANote(path.to_owned());
    let relative = if path.is_absolute() {
        let real = path.canonicalize().map_err(|_| not_a_note())?;
        real.strip_prefix(root.path())
            .map_err(|_| not_a_note())?
            .to_owned()
    } else {
        path.to_owned()
    };
    let note = relative
        .to_str()
        .and_then(|text| VaultPath::new(text).ok())
        .filter(|note| EntryKind::of_file(note) == EntryKind::Note)
        .ok_or_else(not_a_note)?;
    // Canonicalizar resuelve los enlaces de toda la ruta: si cambia, alguno
    // (la nota o una carpeta de por medio) lleva fuera de lo escrito.
    let full = root.path().join(note.as_str());
    let inside = fs::canonicalize(&full).is_ok_and(|real| real == full) && full.is_file();
    if inside { Ok(note) } else { Err(not_a_note()) }
}
