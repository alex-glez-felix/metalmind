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
    #[error("{} ya existe", path.display())]
    Exists { path: PathBuf },
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

/// Como [`write_atomic`], para un archivo que no debe existir: si `path`
/// existe al terminar, aunque otro proceso lo haya creado mientras se
/// escribía, falla sin tocarlo. El paso final es un rename que no pisa
/// (`renameat2` con `RENAME_NOREPLACE` en Linux), así que mirar y escribir
/// son una sola operación.
///
/// # Errors
/// [`FileError::Exists`] si `path` ya existe (también si es un enlace
/// simbólico), o si falla `contents` o cualquier paso de la escritura.
pub fn create_atomic(
    path: &Path,
    contents: impl FnOnce(&mut dyn Write) -> io::Result<()>,
) -> Result<(), FileError> {
    create(path, contents).map_err(|source| {
        if source.kind() == io::ErrorKind::AlreadyExists {
            FileError::Exists {
                path: path.to_owned(),
            }
        } else {
            FileError::Write {
                path: path.to_owned(),
                source,
            }
        }
    })
}

fn create(path: &Path, contents: impl FnOnce(&mut dyn Write) -> io::Result<()>) -> io::Result<()> {
    let dir = parent_dir(path);
    let mut temp = Builder::new()
        .prefix(".nota-")
        .suffix(".tmp")
        .permissions(Permissions::from_mode(NEW_FILE_MODE))
        .tempfile_in(dir)?;
    write_contents(&mut temp, contents)?;
    temp.as_file().sync_all()?;
    temp.persist_noclobber(path).map_err(|error| error.error)?;
    File::open(dir)?.sync_all()
}

/// Como [`write_atomic`], pero sin seguir un enlace: si en `path` apareció
/// uno después de comprobarlo, se reemplaza el enlace y no se escribe fuera
/// del vault. Para lo que ya comprobó que `path` es un archivo.
pub(crate) fn replace_file(path: &Path, text: &str) -> Result<(), FileError> {
    replace_at(path, |out| out.write_all(text.as_bytes())).map_err(|source| FileError::Write {
        path: path.to_owned(),
        source,
    })
}

fn replace(path: &Path, contents: impl FnOnce(&mut dyn Write) -> io::Result<()>) -> io::Result<()> {
    replace_at(&resolve_symlink(path)?, contents)
}

fn replace_at(
    target: &Path,
    contents: impl FnOnce(&mut dyn Write) -> io::Result<()>,
) -> io::Result<()> {
    let dir = parent_dir(target);
    let permissions = existing_permissions(target)?;
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
    temp.persist(target).map_err(|error| error.error)?;
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

#[cfg(test)]
mod tests {
    use std::os::unix::fs::symlink;

    use super::*;

    #[test]
    fn replace_file_replaces_a_link_instead_of_following_it() {
        let vault = tempfile::TempDir::new().unwrap();
        let outside = tempfile::TempDir::new().unwrap();
        let target = outside.path().join("ajeno.md");
        fs::write(&target, "de afuera").unwrap();
        let link = vault.path().join("nota.md");
        symlink(&target, &link).unwrap();

        replace_file(&link, "nuevo").unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "de afuera");
        assert!(
            !fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read_to_string(&link).unwrap(), "nuevo");
    }

    #[test]
    fn create_atomic_never_replaces() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("nota.md");
        fs::write(&path, "del usuario").unwrap();
        let result = create_atomic(&path, |out| out.write_all(b"nuevo"));
        assert!(
            matches!(result, Err(FileError::Exists { .. })),
            "{result:?}"
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "del usuario");
    }
}
