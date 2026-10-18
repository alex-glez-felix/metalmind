use std::fmt;
use std::path::{Component, Path, PathBuf};

/// Ruta de un archivo o carpeta visible del vault, relativa a la raíz, con
/// componentes separados por `/`. La cadena vacía es la raíz.
///
/// Nunca sale de la raíz ni apunta a algo oculto: no admite `..`, `.`,
/// componentes vacíos ni nombres que empiecen con punto.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct VaultPath(String);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("ruta inválida en el vault {path:?}: {reason}")]
pub struct InvalidPath {
    pub path: String,
    pub reason: InvalidReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InvalidReason {
    #[error("es absoluta")]
    Absolute,
    #[error("tiene un componente vacío")]
    EmptyComponent,
    #[error("tiene `.` o `..`")]
    Relative,
    #[error("tiene un componente oculto")]
    Hidden,
    #[error("tiene un byte nulo")]
    Nul,
    #[error("un nombre no puede llevar `/`")]
    Separator,
}

impl VaultPath {
    #[must_use]
    pub fn root() -> Self {
        Self(String::new())
    }

    /// # Errors
    /// Si la ruta no cumple lo de [`VaultPath`].
    pub fn new(path: &str) -> Result<Self, InvalidPath> {
        if path.is_empty() {
            return Ok(Self::root());
        }
        let invalid = |reason| InvalidPath {
            path: path.to_owned(),
            reason,
        };
        if path.starts_with('/') {
            return Err(invalid(InvalidReason::Absolute));
        }
        for component in path.split('/') {
            check_component(component).map_err(invalid)?;
        }
        Ok(Self(path.to_owned()))
    }

    /// La ruta de `path` dentro de `root`, o `None` si está fuera, no es
    /// UTF-8 o no sería válida.
    pub(crate) fn from_fs(root: &Path, path: &Path) -> Option<Self> {
        let relative = path.strip_prefix(root).ok()?;
        let mut parts = Vec::new();
        for component in relative.components() {
            let Component::Normal(name) = component else {
                return None;
            };
            let name = name.to_str()?;
            check_component(name).ok()?;
            parts.push(name);
        }
        Some(Self(parts.join("/")))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    /// El último componente; vacío en la raíz.
    #[must_use]
    pub fn name(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or_default()
    }

    /// `None` solo en la raíz.
    #[must_use]
    pub fn parent(&self) -> Option<Self> {
        if self.is_root() {
            return None;
        }
        Some(match self.0.rfind('/') {
            Some(slash) => Self(self.0[..slash].to_owned()),
            None => Self::root(),
        })
    }

    /// # Errors
    /// Si `name` no es un componente válido (lleva `/`, es oculto, …).
    pub fn join(&self, name: &str) -> Result<Self, InvalidPath> {
        let invalid = |reason| InvalidPath {
            path: name.to_owned(),
            reason,
        };
        if name.contains('/') {
            return Err(invalid(InvalidReason::Separator));
        }
        check_component(name).map_err(invalid)?;
        Ok(self.join_unchecked(name))
    }

    fn join_unchecked(&self, name: &str) -> Self {
        if self.is_root() {
            Self(name.to_owned())
        } else {
            Self(format!("{}/{name}", self.0))
        }
    }

    pub fn components(&self) -> impl Iterator<Item = &str> {
        self.0.split('/').filter(|part| !part.is_empty())
    }

    #[must_use]
    pub fn depth(&self) -> usize {
        self.components().count()
    }

    /// Si `self` es `prefix` o está dentro de él, por componentes.
    #[must_use]
    pub fn starts_with(&self, prefix: &Self) -> bool {
        prefix.is_root()
            || self.0 == prefix.0
            || (self.0.starts_with(&prefix.0)
                && self.0.as_bytes().get(prefix.0.len()) == Some(&b'/'))
    }

    /// `self` con el prefijo `from` cambiado por `to`, si empieza con `from`.
    #[must_use]
    pub fn rebase(&self, from: &Self, to: &Self) -> Option<Self> {
        if !self.starts_with(from) {
            return None;
        }
        let rest = self.0[from.0.len()..].trim_start_matches('/');
        Some(if rest.is_empty() {
            to.clone()
        } else {
            to.join_unchecked(rest)
        })
    }

    /// Las carpetas que contienen a `self`, de la más cercana a la raíz (incluida).
    pub fn ancestors(&self) -> impl Iterator<Item = Self> {
        std::iter::successors(self.parent(), Self::parent)
    }

    pub(crate) fn to_fs(&self, root: &Path) -> PathBuf {
        if self.is_root() {
            root.to_owned()
        } else {
            root.join(&self.0)
        }
    }
}

fn check_component(name: &str) -> Result<(), InvalidReason> {
    match name {
        "" => Err(InvalidReason::EmptyComponent),
        "." | ".." => Err(InvalidReason::Relative),
        _ if name.starts_with('.') => Err(InvalidReason::Hidden),
        _ if name.contains('\0') => Err(InvalidReason::Nul),
        _ => Ok(()),
    }
}

/// Lo que el walk de `ignore` y las operaciones consideran oculto: el vault
/// nunca muestra ni toca nombres con punto (`.obsidian/`, `.git/`, `.trash/`, …).
pub(crate) fn is_hidden_name(name: &str) -> bool {
    name.starts_with('.')
}

impl fmt::Debug for VaultPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl fmt::Display for VaultPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valida_componentes() {
        assert!(VaultPath::new("a/b.md").is_ok());
        assert_eq!(VaultPath::new("").unwrap(), VaultPath::root());
        let reason = |path| VaultPath::new(path).unwrap_err().reason;
        assert_eq!(reason("/a"), InvalidReason::Absolute);
        assert_eq!(reason("a//b"), InvalidReason::EmptyComponent);
        assert_eq!(reason("a/"), InvalidReason::EmptyComponent);
        assert_eq!(reason("a/../b"), InvalidReason::Relative);
        assert_eq!(reason("./a"), InvalidReason::Relative);
        assert_eq!(reason(".obsidian/app.json"), InvalidReason::Hidden);
        assert_eq!(reason("a/.trash"), InvalidReason::Hidden);
        assert_eq!(reason("a\0b"), InvalidReason::Nul);
    }

    #[test]
    fn padre_nombre_y_ancestros() {
        let path = VaultPath::new("a/b/c.md").unwrap();
        assert_eq!(path.name(), "c.md");
        assert_eq!(path.parent().unwrap().as_str(), "a/b");
        let ancestors: Vec<_> = path.ancestors().map(|a| a.0).collect();
        assert_eq!(ancestors, ["a/b", "a", ""]);
        assert_eq!(VaultPath::root().parent(), None);
    }

    #[test]
    fn starts_with_va_por_componentes() {
        let ab = VaultPath::new("a/b").unwrap();
        assert!(ab.starts_with(&VaultPath::new("a").unwrap()));
        assert!(ab.starts_with(&ab));
        assert!(ab.starts_with(&VaultPath::root()));
        assert!(
            !VaultPath::new("a-b")
                .unwrap()
                .starts_with(&VaultPath::new("a").unwrap())
        );
    }

    #[test]
    fn rebase_cambia_el_prefijo() {
        let path = VaultPath::new("a/b/c.md").unwrap();
        let from = VaultPath::new("a/b").unwrap();
        let to = VaultPath::new("x").unwrap();
        assert_eq!(path.rebase(&from, &to).unwrap().as_str(), "x/c.md");
        assert_eq!(from.rebase(&from, &to).unwrap(), to);
        assert_eq!(to.rebase(&from, &to), None);
    }

    #[test]
    fn from_fs_rechaza_lo_que_no_es_del_vault() {
        let root = Path::new("/v");
        let ok = VaultPath::from_fs(root, Path::new("/v/a/b.md")).unwrap();
        assert_eq!(ok.as_str(), "a/b.md");
        assert_eq!(VaultPath::from_fs(root, root).unwrap(), VaultPath::root());
        assert_eq!(VaultPath::from_fs(root, Path::new("/otro/a.md")), None);
        assert_eq!(VaultPath::from_fs(root, Path::new("/v/.git/x")), None);
    }

    #[test]
    fn join_rechaza_separadores() {
        let dir = VaultPath::new("a").unwrap();
        assert_eq!(dir.join("b.md").unwrap().as_str(), "a/b.md");
        assert_eq!(VaultPath::root().join("b.md").unwrap().as_str(), "b.md");
        assert!(dir.join("b/c").is_err());
        assert!(dir.join("..").is_err());
    }
}
