//! Leer un archivo YAML del usuario.

use std::io;
use std::path::{Path, PathBuf};

use crate::diagnostic::Diagnostic;

#[derive(Debug)]
pub(crate) struct YamlFile {
    pub(crate) path: PathBuf,
    pub(crate) text: String,
}

/// El archivo `path`, `None` si no existe.
///
/// # Errors
/// Si el archivo existe pero no se puede leer.
pub(crate) fn read_yaml(path: &Path) -> Result<Option<YamlFile>, Diagnostic> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(YamlFile {
            path: path.to_owned(),
            text,
        })),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(Diagnostic::error(format!("no se pudo leer: {error}")).in_file(path)),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::diagnostic::Severity;

    #[test]
    fn sin_archivo_no_hay_nada() {
        let dir = tempfile::tempdir().unwrap();
        assert!(
            read_yaml(&dir.path().join("config.yaml"))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn lee_el_archivo_con_su_ruta() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        fs::write(&path, "a: 1\n").unwrap();
        let found = read_yaml(&path).unwrap().unwrap();
        assert_eq!(found.path, path);
        assert_eq!(found.text, "a: 1\n");
    }

    #[test]
    fn un_archivo_ilegible_es_error_con_su_ruta() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        fs::create_dir(&path).unwrap();
        let error = read_yaml(&path).unwrap_err();
        assert_eq!(error.severity, Severity::Error);
        assert_eq!(error.file, Some(path));
        assert!(error.message.starts_with("no se pudo leer"));
    }
}
