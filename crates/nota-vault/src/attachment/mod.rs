//! Adjuntos (06 §6): guardar lo que se pega o se arrastra en la carpeta de
//! adjuntos del vault.

mod embed;
mod kind;
mod name;

use std::fs::{File, Permissions};
use std::io::{self, Write as _};
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use jiff::Zoned;
use jiff::civil::DateTime;
use tempfile::{Builder, NamedTempFile};

use crate::index::VaultIndex;
use crate::ops::{self, FileOpError};
use crate::path::VaultPath;

pub use kind::FileKind;

/// La carpeta de adjuntos de un vault que no configura otra.
pub const DEFAULT_ATTACHMENT_FOLDER: &str = "adjuntos";

/// Lo que permite `open(2)` a un archivo nuevo; el umask decide el resto.
const NEW_FILE_MODE: u32 = 0o666;

/// De dónde viene un adjunto; da la base de su nombre.
#[derive(Debug, Clone, Copy)]
pub enum Source<'a> {
    /// Pegado del portapapeles en esta nota: la base es el nombre de la nota.
    Pasted { note: &'a VaultPath },
    /// Un archivo arrastrado: la base es su nombre sin extensión.
    Dropped { file_name: &'a str },
}

/// Un adjunto por guardar.
#[derive(Debug, Clone, Copy)]
pub struct NewAttachment<'a> {
    pub bytes: &'a [u8],
    pub source: Source<'a>,
    /// La fecha y hora local que va en el nombre.
    pub at: DateTime,
}

impl<'a> NewAttachment<'a> {
    /// Un adjunto con la hora local de ahora.
    #[must_use]
    pub fn now(bytes: &'a [u8], source: Source<'a>) -> Self {
        Self {
            bytes,
            source,
            at: Zoned::now().datetime(),
        }
    }
}

/// Dónde quedó un adjunto y el texto que lo embebe en una nota.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedAttachment {
    pub path: VaultPath,
    /// `![[nombre.png]]`, o con la carpeta si el nombre solo no basta.
    pub embed: String,
}

/// Guarda `attachment` en `folder`, que se crea si falta. Nunca pisa nada:
/// el archivo se escribe aparte y aparece de una vez con un nombre libre.
pub(crate) fn save(
    root: &Path,
    index: &VaultIndex,
    folder: &VaultPath,
    attachment: &NewAttachment<'_>,
) -> Result<SavedAttachment, FileOpError> {
    let dir = ops::ensure_folder(root, folder)?;
    let (base, extension) = base_and_extension(attachment);
    let failed = |source| FileOpError::Io {
        action: "guardar el adjunto en",
        path: folder.clone(),
        source,
    };
    let mut temp = write_temp(&dir, attachment.bytes).map_err(failed)?;
    for attempt in 0..u32::MAX {
        let path = folder.join(&name::file_name(&base, attachment.at, &extension, attempt))?;
        match temp.persist_noclobber(path.to_fs(root)) {
            Ok(_) => {
                // Sin esto, un corte de luz puede perder el rename.
                File::open(&dir)
                    .and_then(|dir| dir.sync_all())
                    .map_err(failed)?;
                let embed = embed::embed_text(&path, index);
                return Ok(SavedAttachment { path, embed });
            }
            Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => temp = error.file,
            Err(error) => return Err(failed(error.error)),
        }
    }
    Err(FileOpError::AlreadyExists(folder.clone()))
}

/// La extensión sale del contenido; solo si no se reconoce se usa la del
/// archivo arrastrado.
fn base_and_extension(attachment: &NewAttachment<'_>) -> (String, String) {
    let (base, file_name) = match attachment.source {
        Source::Pasted { note } => (stem(note.name()), None),
        Source::Dropped { file_name } => (stem(file_name), Some(file_name)),
    };
    let extension = match FileKind::detect(attachment.bytes) {
        Some(kind) => kind.extension().to_owned(),
        None => name::extension_or_fallback(file_name),
    };
    (name::base(base), extension)
}

fn stem(file_name: &str) -> &str {
    match file_name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem,
        _ => file_name,
    }
}

/// Un temporal oculto en `dir` con `bytes` ya en disco.
fn write_temp(dir: &Path, bytes: &[u8]) -> io::Result<NamedTempFile> {
    let mut temp = Builder::new()
        .prefix(".nota-")
        .suffix(".tmp")
        .permissions(Permissions::from_mode(NEW_FILE_MODE))
        .tempfile_in(dir)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    Ok(temp)
}
