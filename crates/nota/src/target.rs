//! Qué vault y qué archivo abrir según los argumentos.
//!
//! - `nota <carpeta>`: esa carpeta es el vault, sin archivo abierto.
//! - `nota <archivo>`: el vault es la carpeta que contiene al archivo
//!   (exista o no el archivo, la carpeta tiene que existir).
//! - `nota`: el último vault abierto y, si no hay, el `vault:` de
//!   `config.yaml` (06 §8).
//!
//! Después nota trabaja dentro del vault, como tras un `:cd` de vim: las
//! rutas relativas de `:e`, `:w` y el finder parten de su raíz.

use std::fs;
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

/// `file` (relativa al directorio de trabajo de ahora) como se escribiría
/// desde la raíz `root` del vault: relativa si está dentro, absoluta si no.
/// Se calcula antes de entrar al vault, cuando `file` todavía significa lo
/// que se escribió en la línea de comandos.
pub fn in_vault(root: &Path, file: &Path) -> PathBuf {
    let absolute = std::path::absolute(file).unwrap_or_else(|_| file.to_owned());
    let resolved = match (absolute.parent(), absolute.file_name()) {
        (Some(folder), Some(name)) => {
            fs::canonicalize(folder).map_or_else(|_| absolute.clone(), |folder| folder.join(name))
        }
        _ => absolute.clone(),
    };
    match resolved.strip_prefix(root) {
        Ok(relative) if !relative.as_os_str().is_empty() => relative.to_owned(),
        _ => absolute,
    }
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

    #[test]
    fn el_archivo_se_ve_desde_la_raiz_del_vault() {
        let dir = vault();
        let root = fs::canonicalize(dir.path().join("notas")).unwrap();
        fs::create_dir(root.join("sub")).unwrap();
        let roundabout = dir.path().join("notas/sub/../a.md");
        assert_eq!(in_vault(&root, &roundabout), PathBuf::from("a.md"));
        let new = dir.path().join("notas/sub/nueva.md");
        assert_eq!(in_vault(&root, &new), PathBuf::from("sub/nueva.md"));
        let outside = dir.path().join("fuera.md");
        assert_eq!(in_vault(&root, &outside), outside);
    }
}
