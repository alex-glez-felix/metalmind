//! Qué vault y qué archivo abrir según los argumentos.
//!
//! - `metalmind <carpeta>`: esa carpeta es el vault, sin archivo abierto.
//! - `metalmind <archivo>`: el vault es la carpeta que contiene al archivo
//!   (exista o no el archivo, la carpeta tiene que existir). Una ruta que no
//!   existe es un archivo solo si es una nota (`nueva.md`); si no
//!   (`~/Notas`, `~/Notas.2026`) es una carpeta que falta, y se dice en vez
//!   de abrir la de arriba.
//! - `metalmind`: la carpeta de la variable `METALMIND_VAULT`, si está; si no, el
//!   último vault abierto y, si no hay, el `vault:` de `config.yaml`
//!   (06 §8).
//!
//! Después Metalmind trabaja dentro del vault, como tras un `:cd` de vim: las
//! rutas relativas de `:e`, `:w` y el finder parten de su raíz.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use metalmind_config::{Paths, Settings};
use metalmind_syntax::note_name;

/// La variable de entorno con el vault que abre `metalmind` sin argumentos.
pub const VAULT_VARIABLE: &str = "METALMIND_VAULT";

#[derive(Debug, PartialEq, Eq)]
pub struct Target {
    pub vault: PathBuf,
    pub file: Option<PathBuf>,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum TargetError {
    #[error("no existe la carpeta {}", .0.display())]
    MissingFolder(PathBuf),
    #[error(
        "no existe la carpeta {}; créala con `mkdir -p {}` o pasa una que exista",
        .0.display(),
        shell_argument(.0)
    )]
    MissingVault(PathBuf),
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
            remembered: paths.and_then(metalmind_ui::last_vault),
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
    if !path.exists() && !is_note(path) {
        return Err(TargetError::MissingVault(path.to_owned()));
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

fn is_note(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(note_name::is_note)
}

/// `path` como se escribe en el shell para que llegue tal cual a un comando
/// como argumento, no como opción.
fn shell_argument(path: &Path) -> String {
    let path = path.to_string_lossy();
    let path = if path.starts_with('-') {
        format!("./{path}")
    } else {
        path.into_owned()
    };
    let plain = |c: char| c.is_ascii_alphanumeric() || "/._-+,:@%".contains(c);
    if !path.is_empty() && path.chars().all(plain) {
        path
    } else {
        format!("'{}'", path.replace('\'', r"'\''"))
    }
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

    /// #97: `metalmind ~/Notas` sin `~/Notas` abría `~` entero como vault.
    #[test]
    fn una_carpeta_que_no_existe_no_abre_la_de_arriba() {
        let dir = vault();
        for missing in [
            "Notas",
            "Notas/",
            "notas/no/existe",
            "Notas.2026",
            "vault.v2/",
        ] {
            let path = dir.path().join(missing);
            assert_eq!(
                resolve(Some(&path), None, None, None),
                Err(TargetError::MissingVault(path.clone())),
                "{missing}"
            );
        }
    }

    #[test]
    fn la_carpeta_que_falta_se_nombra_con_como_crearla() {
        let message = TargetError::MissingVault(PathBuf::from("/x/Notas")).to_string();
        assert_eq!(
            message,
            "no existe la carpeta /x/Notas; créala con `mkdir -p /x/Notas` o pasa una que exista"
        );
        let message = TargetError::MissingVault(PathBuf::from("/x/Mi vault's")).to_string();
        assert_eq!(
            message,
            "no existe la carpeta /x/Mi vault's; créala con `mkdir -p '/x/Mi vault'\\''s'` o pasa una que exista"
        );
        let message = TargetError::MissingVault(PathBuf::from("-notas")).to_string();
        assert_eq!(
            message,
            "no existe la carpeta -notas; créala con `mkdir -p ./-notas` o pasa una que exista"
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

    /// Alex, 06 §8: argumento → `METALMIND_VAULT` → último vault → `vault:`.
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
