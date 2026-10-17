use std::fs::{self, File, Permissions};
use std::io::{self, BufWriter, Write};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use tempfile::{Builder, NamedTempFile};

/// Lo que pide `open(2)` para un archivo nuevo; el umask decide el resto.
const NEW_FILE_MODE: u32 = 0o666;

#[derive(Debug, thiserror::Error)]
pub enum FileError {
    #[error("no se pudo leer {}: {source}", path.display())]
    Read { path: PathBuf, source: io::Error },
    #[error("{} no es texto UTF-8", path.display())]
    NotUtf8 { path: PathBuf },
    #[error("no se pudo guardar {}: {source}", path.display())]
    Write { path: PathBuf, source: io::Error },
}

/// Contenido de la nota, o `None` si todavía no existe.
///
/// # Errors
/// Si no se puede leer o no es UTF-8.
pub fn read(path: &Path) -> Result<Option<String>, FileError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(FileError::Read {
                path: path.to_owned(),
                source,
            });
        }
    };
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| FileError::NotUtf8 {
            path: path.to_owned(),
        })
}

/// Reemplaza `path` con lo que escriba `contents`, sin dejarlo nunca a
/// medias: escribe un temporal en el mismo directorio, lo baja a disco y lo
/// renombra encima. Conserva los permisos del original y, si `path` es un
/// enlace simbólico, escribe en su destino. Si algo falla, el original queda
/// intacto y el temporal se borra.
///
/// # Errors
/// Si falla `contents` o cualquier paso de la escritura.
pub fn write_atomic(
    path: &Path,
    contents: impl FnOnce(&mut dyn Write) -> io::Result<()>,
) -> Result<(), FileError> {
    replace(path, contents).map_err(|source| FileError::Write {
        path: path.to_owned(),
        source,
    })
}

fn replace(path: &Path, contents: impl FnOnce(&mut dyn Write) -> io::Result<()>) -> io::Result<()> {
    let target = resolve_symlink(path)?;
    let dir = parent_dir(&target);
    let permissions = existing_permissions(&target)?;
    let mut temp = Builder::new()
        .prefix(".nota-")
        .suffix(".tmp")
        .permissions(
            permissions
                .clone()
                .unwrap_or_else(|| Permissions::from_mode(NEW_FILE_MODE)),
        )
        .tempfile_in(dir)?;
    write_contents(&mut temp, contents)?;
    // Al crear, el umask recorta los permisos; los del original van tal cual.
    if let Some(permissions) = permissions {
        temp.as_file().set_permissions(permissions)?;
    }
    temp.as_file().sync_all()?;
    temp.persist(&target).map_err(|error| error.error)?;
    // Sin esto, un corte de luz puede perder el rename aunque el contenido esté en disco.
    File::open(dir)?.sync_all()
}

fn write_contents(
    temp: &mut NamedTempFile,
    contents: impl FnOnce(&mut dyn Write) -> io::Result<()>,
) -> io::Result<()> {
    let mut writer = BufWriter::new(temp.as_file_mut());
    contents(&mut writer)?;
    writer.flush()
}

fn resolve_symlink(path: &Path) -> io::Result<PathBuf> {
    match fs::canonicalize(path) {
        Ok(real) => Ok(real),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(path.to_owned()),
        Err(error) => Err(error),
    }
}

fn parent_dir(path: &Path) -> &Path {
    match path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir,
        _ => Path::new("."),
    }
}

fn existing_permissions(path: &Path) -> io::Result<Option<Permissions>> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(Some(metadata.permissions())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}
