//! Qué vault y qué archivo abrir según los argumentos.
//!
//! - `nota <carpeta>`: esa carpeta es el vault, sin archivo abierto.
//! - `nota <archivo>`: el vault es la carpeta que contiene al archivo
//!   (exista o no el archivo, la carpeta tiene que existir).
//! - `nota`: el último vault abierto y, si no hay, el `vault:` de
//!   `config.yaml` (06 §8).

use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub struct Target {
    pub vault: PathBuf,
    pub file: Option<PathBuf>,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum TargetError {
    #[error("no existe la carpeta {}", .0.display())]
    MissingFolder(PathBuf),
    #[error("no hay un vault que abrir: pasa una carpeta o un archivo")]
    NoVault,
}

/// `remembered` es el último vault abierto y `configured` el de la config;
/// de esos se usa el primero que todavía sea una carpeta.
pub fn resolve(
    argument: Option<&Path>,
    remembered: Option<PathBuf>,
    configured: Option<PathBuf>,
) -> Result<Target, TargetError> {
    let Some(path) = argument else {
        return remembered
            .into_iter()
            .chain(configured)
            .find(|vault| vault.is_dir())
            .map(|vault| Target { vault, file: None })
            .ok_or(TargetError::NoVault);
    };
    if path.is_dir() {
        return Ok(Target {
            vault: path.to_owned(),
            file: None,
        });
    }
    let folder = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_owned(),
        _ => PathBuf::from("."),
    };
    if !folder.is_dir() {
        return Err(TargetError::MissingFolder(folder));
    }
    Ok(Target {
        vault: folder,
        file: Some(path.to_owned()),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    fn vault() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("notas")).unwrap();
        fs::write(dir.path().join("notas/a.md"), "a\n").unwrap();
        dir
    }

    #[test]
    fn una_carpeta_es_el_vault() {
        let dir = vault();
        let notas = dir.path().join("notas");
        assert_eq!(
            resolve(Some(&notas), None, None),
            Ok(Target {
                vault: notas,
                file: None
            })
        );
    }

    #[test]
    fn un_archivo_abre_el_vault_de_su_carpeta() {
        let dir = vault();
        for name in ["a.md", "nueva.md"] {
            let file = dir.path().join("notas").join(name);
            assert_eq!(
                resolve(Some(&file), None, None),
                Ok(Target {
                    vault: dir.path().join("notas"),
                    file: Some(file),
                })
            );
        }
        assert_eq!(
            resolve(Some(Path::new("a.md")), None, None),
            Ok(Target {
                vault: PathBuf::from("."),
                file: Some(PathBuf::from("a.md")),
            })
        );
    }

    #[test]
    fn la_carpeta_del_archivo_tiene_que_existir() {
        let dir = vault();
        let file = dir.path().join("no/existe.md");
        assert_eq!(
            resolve(Some(&file), None, None),
            Err(TargetError::MissingFolder(dir.path().join("no")))
        );
    }

    #[test]
    fn sin_argumento_el_ultimo_vault_y_despues_el_de_la_config() {
        let dir = vault();
        let notas = dir.path().join("notas");
        let gone = dir.path().join("borrado");
        let target = |vault: &Path| {
            Ok(Target {
                vault: vault.to_owned(),
                file: None,
            })
        };
        assert_eq!(
            resolve(None, Some(notas.clone()), Some(dir.path().to_owned())),
            target(&notas)
        );
        assert_eq!(
            resolve(None, Some(gone.clone()), Some(notas.clone())),
            target(&notas)
        );
        assert_eq!(resolve(None, None, Some(gone)), Err(TargetError::NoVault));
        assert_eq!(resolve(None, None, None), Err(TargetError::NoVault));
    }
}
