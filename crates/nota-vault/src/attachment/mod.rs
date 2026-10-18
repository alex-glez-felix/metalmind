//! Adjuntos (06 §6): guardar lo que se pega o se arrastra en la carpeta de
//! adjuntos del vault. Qué adjuntos no usa ninguna nota lo dice el índice de
//! links ([`crate::LinkIndex::orphan_attachments`]).

mod kind;
mod name;

use std::fs;
use std::io::Write;
use std::os::unix::fs::MetadataExt as _;
use std::path::Path;

use jiff::Zoned;
use jiff::civil::DateTime;

use crate::file::{FileError, create_atomic};
use crate::index::EntryKind;
use crate::links::LinkIndex;
use crate::ops::{self, FileOpError};
use crate::path::{InvalidPath, VaultPath};
use crate::scan::scan_partial;

pub use kind::FileKind;

/// La carpeta de adjuntos de un vault que no configura otra.
pub const DEFAULT_ATTACHMENTS_FOLDER: &str = "adjuntos";

/// Un adjunto por guardar.
#[derive(Debug, Clone, Copy)]
pub struct NewAttachment<'a> {
    pub bytes: &'a [u8],
    /// La nota donde va el embed: desde ella se resuelve y, en un pegado, da
    /// la base del nombre.
    pub note: &'a VaultPath,
    /// El nombre del archivo arrastrado; `None` si se pegó.
    pub file_name: Option<&'a str>,
    /// La fecha y hora local que va en el nombre.
    pub at: DateTime,
}

impl<'a> NewAttachment<'a> {
    /// Lo pegado en `note`, con la hora local de ahora.
    #[must_use]
    pub fn pasted(bytes: &'a [u8], note: &'a VaultPath) -> Self {
        Self {
            bytes,
            note,
            file_name: None,
            at: Zoned::now().datetime(),
        }
    }

    /// El archivo `file_name` arrastrado a `note`, con la hora local de ahora.
    #[must_use]
    pub fn dropped(bytes: &'a [u8], file_name: &'a str, note: &'a VaultPath) -> Self {
        Self {
            file_name: Some(file_name),
            ..Self::pasted(bytes, note)
        }
    }
}

/// Dónde quedó un adjunto y el texto que lo embebe en su nota.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedAttachment {
    pub path: VaultPath,
    /// `![[nombre.png]]`, o con las carpetas que hagan falta para no empatar.
    pub embed: String,
}

#[derive(Debug, thiserror::Error)]
pub enum AttachmentError {
    /// Un `.md` arrastrado es una nota: en la carpeta de adjuntos sería una
    /// nota más, no un adjunto.
    #[error("{0} sería una nota, no un adjunto")]
    Note(String),
    /// Los `.gitignore` o `.ignore` del vault lo esconden: el índice no lo
    /// vería y su embed no resolvería. Como en todos los errores, no queda
    /// nada en disco.
    #[error("{0} queda fuera del vault por sus reglas de ignorados")]
    Ignored(VaultPath),
    /// Un `#`, `|` o `]]` en la carpeta: ningún `![[...]]` llega a él.
    #[error("{0} no se puede nombrar en un ![[...]]")]
    Unnameable(VaultPath),
    #[error("ya hay demasiados adjuntos llamados {0}")]
    NameTaken(String),
    #[error(transparent)]
    InvalidName(#[from] InvalidPath),
    #[error(transparent)]
    Folder(#[from] FileOpError),
    #[error(transparent)]
    Write(FileError),
}

/// `nombre.png`, `nombre-1.png`, … hasta aquí.
const MAX_ATTEMPT: u32 = 999;

/// Guarda `attachment` en `folder`, que se crea si falta. Nunca pisa nada: el
/// archivo se escribe aparte y aparece de una vez con un nombre libre.
pub(crate) fn save(
    root: &Path,
    links: &LinkIndex,
    folder: &VaultPath,
    attachment: &NewAttachment<'_>,
) -> Result<SavedAttachment, AttachmentError> {
    let (base, extension) = base_and_extension(attachment);
    let first = folder.join(&name::file_name(&base, attachment.at, &extension, 0))?;
    if EntryKind::of_file(&first) == EntryKind::Note {
        return Err(AttachmentError::Note(first.name().to_owned()));
    }
    let created = ops::create_parents(root, &first)?;
    let saved = create_unique(root, links, folder, &base, &extension, attachment)
        .and_then(|created| visible(root, created));
    if saved.is_err() {
        ops::remove_empty(root, &created);
    }
    saved
}

/// Un adjunto recién creado y qué archivo es, para no borrar otro en su lugar.
struct Created {
    saved: SavedAttachment,
    identity: Option<(u64, u64)>,
}

/// Crea el adjunto con el primer nombre libre. Que un wikilink pueda
/// nombrarlo se sabe antes de escribir: los nombres del índice no cambian.
fn create_unique(
    root: &Path,
    links: &LinkIndex,
    folder: &VaultPath,
    base: &str,
    extension: &str,
    attachment: &NewAttachment<'_>,
) -> Result<Created, AttachmentError> {
    for attempt in 0..=MAX_ATTEMPT {
        let path = folder.join(&name::file_name(base, attachment.at, extension, attempt))?;
        let embed = links
            .embed_for_new(attachment.note, &path)
            .ok_or_else(|| AttachmentError::Unnameable(path.clone()))?;
        let full = path.to_fs(root);
        let write = |out: &mut dyn Write| out.write_all(attachment.bytes);
        match create_atomic(&full, write) {
            Ok(()) => {
                let identity = fs::symlink_metadata(&full)
                    .ok()
                    .map(|metadata| (metadata.dev(), metadata.ino()));
                return Ok(Created {
                    saved: SavedAttachment { path, embed },
                    identity,
                });
            }
            Err(FileError::Exists { .. }) => {}
            Err(error) => return Err(AttachmentError::Write(error)),
        }
    }
    Err(AttachmentError::NameTaken(name::file_name(
        base,
        attachment.at,
        extension,
        0,
    )))
}

/// Si el índice no lo va a ver, no sirve: se borra, pero solo si la ruta
/// sigue siendo el archivo que se creó. Las reglas de ignorados solo se
/// aplican a lo que existe, así que no se puede saber antes de crearlo.
fn visible(root: &Path, created: Created) -> Result<SavedAttachment, AttachmentError> {
    let path = &created.saved.path;
    if is_visible(root, path) {
        return Ok(created.saved);
    }
    let full = path.to_fs(root);
    let same = fs::symlink_metadata(&full)
        .ok()
        .map(|metadata| (metadata.dev(), metadata.ino()));
    if same.is_some() && same == created.identity {
        if let Err(error) = fs::remove_file(&full) {
            log::warn!("no se pudo quitar el adjunto ignorado {path}: {error}");
        }
    } else {
        log::warn!("{path} cambió antes de quitarlo por ignorado: se deja");
    }
    Err(AttachmentError::Ignored(path.clone()))
}

/// Si el walk del vault ve `path`: el mismo que arma el índice.
fn is_visible(root: &Path, path: &VaultPath) -> bool {
    scan_partial(root, std::slice::from_ref(path))
        .entries
        .iter()
        .any(|(found, _)| found == path)
}

/// La extensión sale del contenido; la del archivo arrastrado gana si el
/// contenido no se reconoce o su firma la comparten otros formatos.
fn base_and_extension(attachment: &NewAttachment<'_>) -> (String, String) {
    let own = attachment.file_name.and_then(name::extension_of);
    let extension = match (FileKind::detect(attachment.bytes), own) {
        (Some(kind), Some(own)) if kind.shares_signature() => own,
        (Some(kind), _) => kind.extension().to_owned(),
        (None, own) => own.unwrap_or_else(|| name::FALLBACK_EXTENSION.to_owned()),
    };
    let base = match attachment.file_name {
        Some(file_name) => stem(file_name),
        None => stem(attachment.note.name()),
    };
    (name::base(base), extension)
}

fn stem(file_name: &str) -> &str {
    match file_name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem,
        _ => file_name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ignored(identity: impl Fn(&Path) -> Option<(u64, u64)>) -> (tempfile::TempDir, bool) {
        let dir = tempfile::TempDir::new().unwrap();
        fs::write(dir.path().join(".gitignore"), "*.png\n").unwrap();
        let full = dir.path().join("x.png");
        fs::write(&full, "otro").unwrap();
        let created = Created {
            saved: SavedAttachment {
                path: VaultPath::new("x.png").unwrap(),
                embed: "![[x.png]]".to_owned(),
            },
            identity: identity(&full),
        };
        let result = visible(dir.path(), created);
        assert!(
            matches!(result, Err(AttachmentError::Ignored(_))),
            "{result:?}"
        );
        let kept = full.exists();
        (dir, kept)
    }

    #[test]
    fn un_adjunto_ignorado_se_quita_si_sigue_siendo_el_mismo() {
        let same = |full: &Path| {
            let metadata = fs::symlink_metadata(full).unwrap();
            Some((metadata.dev(), metadata.ino()))
        };
        assert!(!ignored(same).1);
    }

    #[test]
    fn un_archivo_que_ocupo_su_lugar_no_se_borra() {
        assert!(ignored(|_| Some((0, 0))).1);
        assert!(ignored(|_| None).1);
    }
}
