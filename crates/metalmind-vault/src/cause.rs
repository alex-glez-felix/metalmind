//! Por qué falló una operación sobre un archivo, sin su ruta: para avisarlo
//! donde la ruta ya se ve (la barra de estado muestra la nota a la izquierda),
//! sin que una ruta larga tape la causa.

use std::io;

use metalmind_syntax::note_name;

use crate::file::FileError;
use crate::ops::FileOpError;
use crate::vault::SaveError;

impl FileError {
    #[must_use]
    pub fn cause(&self) -> String {
        match self {
            Self::Read { source, .. } | Self::Write { source, .. } => io_cause(source),
            Self::NotUtf8 { .. } => "no es texto UTF-8".to_owned(),
            Self::Exists { .. } => "ya existe".to_owned(),
        }
    }
}

impl FileOpError {
    #[must_use]
    pub fn cause(&self) -> String {
        match self {
            Self::Io { source, .. } => io_cause(source),
            Self::NotANote(_) => format!(
                "no es una nota: tiene que terminar en {}",
                note_name::EXTENSION
            ),
            Self::Symlink(_) => "pasa por un enlace simbólico".to_owned(),
            Self::NotAFolder(dir) => format!("{dir} es un archivo, no una carpeta"),
            Self::AlreadyExists(_) => "ya existe".to_owned(),
            Self::NotFound(_) => "no existe".to_owned(),
            Self::Root | Self::TrashIsSymlink | Self::IntoItself(_) => self.to_string(),
        }
    }
}

impl SaveError {
    #[must_use]
    pub fn cause(&self) -> String {
        match self {
            Self::Path(error) => error.cause(),
            Self::Write(error) => error.cause(),
        }
    }
}

/// Lo que dice el sistema, en las palabras de la interfaz; sin la ruta que
/// agregan `tempfile` y compañía al mensaje.
fn io_cause(error: &io::Error) -> String {
    match error.kind() {
        io::ErrorKind::PermissionDenied => "permiso denegado".to_owned(),
        io::ErrorKind::NotFound => "la carpeta no existe".to_owned(),
        io::ErrorKind::StorageFull => "el disco está lleno".to_owned(),
        io::ErrorKind::QuotaExceeded => "se acabó la cuota de disco".to_owned(),
        io::ErrorKind::ReadOnlyFilesystem => "el disco es de solo lectura".to_owned(),
        io::ErrorKind::NotADirectory => "una carpeta de la ruta es un archivo".to_owned(),
        io::ErrorKind::IsADirectory => "es una carpeta".to_owned(),
        kind => kind.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use pretty_assertions::assert_eq;

    use super::*;

    fn write_error(kind: io::ErrorKind) -> FileError {
        FileError::Write {
            path: PathBuf::from("/una/ruta/muy/larga/nota.md"),
            source: io::Error::new(kind, "detalle del sistema at path \"/tmp/.tmpXYZ\""),
        }
    }

    #[test]
    fn la_causa_no_lleva_la_ruta() {
        let error = write_error(io::ErrorKind::PermissionDenied);
        assert!(error.to_string().contains("/una/ruta"));
        assert_eq!(error.cause(), "permiso denegado");
        assert_eq!(SaveError::Write(error).cause(), "permiso denegado");
    }

    #[test]
    fn cada_error_comun_tiene_su_causa() {
        let cases = [
            (io::ErrorKind::NotFound, "la carpeta no existe"),
            (io::ErrorKind::StorageFull, "el disco está lleno"),
            (
                io::ErrorKind::ReadOnlyFilesystem,
                "el disco es de solo lectura",
            ),
        ];
        for (kind, cause) in cases {
            assert_eq!(write_error(kind).cause(), cause, "{kind:?}");
        }
        let other = write_error(io::ErrorKind::Interrupted).cause();
        assert!(!other.contains("/tmp"), "{other}");
    }
}
