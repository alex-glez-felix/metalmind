//! Adjuntos (06 §6): guardar lo que se pega o se arrastra en la carpeta de
//! adjuntos del vault. Qué adjuntos no usa ninguna nota lo dice el índice de
//! links ([`crate::LinkIndex::orphan_attachments`]).

mod kind;
mod name;

use std::io::Write;
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
    let saved = create_unique(root, folder, &base, &extension, attachment).and_then(|path| {
        let saved = usable(root, links, attachment.note, &path);
        // Es el archivo que se acaba de crear: nadie más lo conoce todavía.
        if saved.is_err()
            && let Err(error) = std::fs::remove_file(path.to_fs(root))
        {
            log::warn!("no se pudo quitar el adjunto {path}: {error}");
        }
        saved
    });
    if saved.is_err() {
        ops::remove_empty(root, &created);
    }
    saved
}

/// El adjunto ya en disco, si el índice lo va a ver y la nota puede
/// nombrarlo; si no, no serviría.
fn usable(
    root: &Path,
    links: &LinkIndex,
    note: &VaultPath,
    path: &VaultPath,
) -> Result<SavedAttachment, AttachmentError> {
    if !is_visible(root, path) {
        return Err(AttachmentError::Ignored(path.clone()));
    }
    let embed = links
        .embed_for_new(note, path)
        .ok_or_else(|| AttachmentError::Unnameable(path.clone()))?;
    Ok(SavedAttachment {
        path: path.clone(),
        embed,
    })
}

fn create_unique(
    root: &Path,
    folder: &VaultPath,
    base: &str,
    extension: &str,
    attachment: &NewAttachment<'_>,
) -> Result<VaultPath, AttachmentError> {
    for attempt in 0..=MAX_ATTEMPT {
        let path = folder.join(&name::file_name(base, attachment.at, extension, attempt))?;
        let write = |out: &mut dyn Write| out.write_all(attachment.bytes);
        match create_atomic(&path.to_fs(root), write) {
            Ok(()) => return Ok(path),
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
