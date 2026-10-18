//! Qué vault y qué archivo abrir según los argumentos.
//!
//! - `nota <carpeta>`: esa carpeta es el vault, sin archivo abierto.
//! - `nota <archivo>`: el vault es la carpeta que contiene al archivo
//!   (exista o no el archivo, la carpeta tiene que existir).
//! - `nota`: la carpeta de la variable `NOTA_VAULT`, si está; si no, el
//!   último vault abierto y, si no hay, el `vault:` de `config.yaml`
//!   (06 §8).
//!
//! Después nota trabaja dentro del vault, como tras un `:cd` de vim: las
//! rutas relativas de `:e`, `:w` y el finder parten de su raíz.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use nota_config::{Paths, Settings};

/// La variable de entorno con el vault que abre `nota` sin argumentos.
pub const VAULT_VARIABLE: &str = "NOTA_VAULT";

#[derive(Debug, PartialEq, Eq)]
pub struct Target {
    pub vault: PathBuf,
    pub file: Option<PathBuf>,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum TargetError {
    #[error("no existe la carpeta {}", .0.display())]
    MissingFolder(PathBuf),
    #[error("{VAULT_VARIABLE} apunta a {}, que no es una carpeta", .0.display())]
    VariableNotAFolder(PathBuf),
    #[error("no hay un vault que abrir: pasa una carpeta o un archivo")]
    NoVault,
}

/// El vault de [`VAULT_VARIABLE`], con el valor que tenga en el entorno;
/// vacía cuenta como no puesta.
#[must_use]
pub fn from_variable(value: Option<OsString>) -> Option<PathBuf> {
    value.filter(|value| !value.is_empty()).map(PathBuf::from)
}

/// De dónde puede salir el vault cuando no se nombra uno.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Sources {
    /// El de [`VAULT_VARIABLE`].
    pub variable: Option<PathBuf>,
    /// El último vault abierto.
    pub remembered: Option<PathBuf>,
    /// El `vault:` de la config.
    pub configured: Option<PathBuf>,
}

impl Sources {
    /// Las del entorno de este proceso; sin `paths` no hay último vault.
    pub fn detect(paths: Option<&Paths>, settings: &Settings) -> Self {
        Self {
            variable: from_variable(std::env::var_os(VAULT_VARIABLE)),
            remembered: paths.and_then(nota_ui::last_vault),
            configured: dirs::home_dir().and_then(|home| settings.vault(&home)),
        }
    }
}

/// El vault cuando no se nombra uno: `variable`, `remembered` y
/// `configured`, en ese orden. La variable se eligió a propósito: si no
/// sirve, es un error. De los otros dos se usa el primero que todavía sea
/// una carpeta.
pub fn default_vault(sources: Sources) -> Result<PathBuf, TargetError> {
    let Sources {
        variable,
        remembered,
        configured,
    } = sources;
    if let Some(vault) = variable {
        if !vault.is_dir() {
            return Err(TargetError::VariableNotAFolder(vault));
        }
        return Ok(vault);
    }
    remembered
        .into_iter()
        .chain(configured)
        .find(|vault| vault.is_dir())
        .ok_or(TargetError::NoVault)
}

/// El argumento, si lo hay, y si no [`default_vault`].
pub fn resolve(argument: Option<&Path>, sources: Sources) -> Result<Target, TargetError> {
    let Some(path) = argument else {
        return default_vault(sources).map(|vault| Target { vault, file: None });
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

    fn resolve(
        argument: Option<&Path>,
        variable: Option<PathBuf>,
        remembered: Option<PathBuf>,
        configured: Option<PathBuf>,
    ) -> Result<Target, TargetError> {
        super::resolve(
            argument,
            Sources {
                variable,
                remembered,
                configured,
            },
        )
    }

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
            resolve(Some(&notas), None, None, None),
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
                resolve(Some(&file), None, None, None),
                Ok(Target {
                    vault: dir.path().join("notas"),
                    file: Some(file),
                })
            );
        }
        assert_eq!(
            resolve(Some(Path::new("a.md")), None, None, None),
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
            resolve(Some(&file), None, None, None),
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
            resolve(None, None, Some(notas.clone()), Some(dir.path().to_owned())),
            target(&notas)
        );
        assert_eq!(
            resolve(None, None, Some(gone.clone()), Some(notas.clone())),
            target(&notas)
        );
        assert_eq!(
            resolve(None, None, None, Some(gone)),
            Err(TargetError::NoVault)
        );
        assert_eq!(resolve(None, None, None, None), Err(TargetError::NoVault));
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

    /// Alex, 06 §8: argumento → `NOTA_VAULT` → último vault → `vault:`.
    #[test]
    fn la_variable_va_despues_del_argumento_y_antes_que_lo_recordado() {
        let dir = vault();
        let notas = dir.path().join("notas");
        let other = TempDir::new().unwrap();
        let from_env = || Some(other.path().to_owned());
        let target = |vault: &Path, file: Option<PathBuf>| {
            Ok(Target {
                vault: vault.to_owned(),
                file,
            })
        };
        let a = notas.join("a.md");
        assert_eq!(
            resolve(Some(&a), from_env(), None, None),
            target(&notas, Some(a.clone())),
            "el argumento gana"
        );
        assert_eq!(
            resolve(None, from_env(), Some(notas.clone()), Some(notas.clone())),
            target(other.path(), None),
            "la variable gana a lo recordado y a la config"
        );
        let gone = dir.path().join("borrado");
        assert_eq!(
            resolve(None, Some(gone.clone()), Some(notas.clone()), None),
            Err(TargetError::VariableNotAFolder(gone)),
            "una variable que no sirve es un error, no se salta"
        );
        assert_eq!(
            resolve(None, Some(a.clone()), None, None),
            Err(TargetError::VariableNotAFolder(a))
        );
    }

    #[test]
    fn la_variable_vacia_cuenta_como_no_puesta() {
        assert_eq!(from_variable(None), None);
        assert_eq!(from_variable(Some(OsString::new())), None);
        assert_eq!(
            from_variable(Some(OsString::from("/notas"))),
            Some(PathBuf::from("/notas"))
        );
    }
}
