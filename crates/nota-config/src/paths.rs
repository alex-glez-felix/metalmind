//! Dónde vive cada cosa: config, datos, caché y estado.
//!
//! En Linux (y los demás Unix salvo macOS) se sigue la especificación XDG a
//! mano para poder probarla sin tocar el entorno del proceso; en macOS y
//! Windows se usa lo que dice `dirs` para cada plataforma.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

const APP: &str = "nota";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paths {
    pub config: PathBuf,
    pub data: PathBuf,
    pub cache: PathBuf,
    /// Lo que debe sobrevivir a un reinicio pero no es del usuario: journal de
    /// recuperación, historial.
    pub state: PathBuf,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("no se encontró el directorio del usuario para ubicar la configuración")]
pub struct NoHomeDir;

impl Paths {
    /// Las rutas de la plataforma actual.
    ///
    /// # Errors
    /// [`NoHomeDir`] si no hay de dónde sacar el directorio del usuario.
    #[cfg(not(any(target_os = "macos", windows)))]
    pub fn detect() -> Result<Self, NoHomeDir> {
        let home = dirs::home_dir().ok_or(NoHomeDir)?;
        Ok(Self::from_xdg(|name| std::env::var_os(name), &home))
    }

    /// Las rutas de la plataforma actual.
    ///
    /// # Errors
    /// [`NoHomeDir`] si no hay de dónde sacar el directorio del usuario.
    #[cfg(any(target_os = "macos", windows))]
    pub fn detect() -> Result<Self, NoHomeDir> {
        let data = dirs::data_local_dir().ok_or(NoHomeDir)?.join(APP);
        Ok(Self {
            config: dirs::config_dir().ok_or(NoHomeDir)?.join(APP),
            cache: dirs::cache_dir().ok_or(NoHomeDir)?.join(APP),
            state: data.join("state"),
            data,
        })
    }

    /// Rutas según XDG Base Directory: `$XDG_*_HOME` si es absoluta, si no el
    /// default bajo `home`. `env` lee una variable de entorno.
    pub fn from_xdg(env: impl Fn(&str) -> Option<OsString>, home: &Path) -> Self {
        let base = |variable: &str, default: &str| {
            env(variable)
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .unwrap_or_else(|| home.join(default))
                .join(APP)
        };
        Self {
            config: base("XDG_CONFIG_HOME", ".config"),
            data: base("XDG_DATA_HOME", ".local/share"),
            cache: base("XDG_CACHE_HOME", ".cache"),
            state: base("XDG_STATE_HOME", ".local/state"),
        }
    }

    /// Todo bajo una misma carpeta; para tests y para correr aislado.
    #[must_use]
    pub fn under(root: &Path) -> Self {
        Self {
            config: root.join("config"),
            data: root.join("data"),
            cache: root.join("cache"),
            state: root.join("state"),
        }
    }

    #[must_use]
    pub fn config_file(&self) -> PathBuf {
        self.config.join("config.yaml")
    }

    #[must_use]
    pub fn themes_dir(&self) -> PathBuf {
        self.config.join("themes")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use pretty_assertions::assert_eq;

    use super::*;

    fn env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let vars: HashMap<String, OsString> = vars
            .iter()
            .map(|&(name, value)| (name.to_owned(), value.into()))
            .collect();
        move |name| vars.get(name).cloned()
    }

    #[test]
    fn sin_variables_usa_los_defaults_bajo_home() {
        let home = tempfile::tempdir().unwrap();
        let paths = Paths::from_xdg(env(&[]), home.path());
        assert_eq!(
            paths,
            Paths {
                config: home.path().join(".config/nota"),
                data: home.path().join(".local/share/nota"),
                cache: home.path().join(".cache/nota"),
                state: home.path().join(".local/state/nota"),
            }
        );
        assert_eq!(
            paths.config_file(),
            home.path().join(".config/nota/config.yaml")
        );
        assert_eq!(paths.themes_dir(), home.path().join(".config/nota/themes"));
    }

    #[test]
    fn respeta_las_variables_xdg() {
        let home = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let under = |name: &str| other.path().join(name);
        let text = |name: &str| under(name).to_str().unwrap().to_owned();
        let (config, data, cache, state) = (text("c"), text("d"), text("k"), text("s"));
        let paths = Paths::from_xdg(
            env(&[
                ("XDG_CONFIG_HOME", &config),
                ("XDG_DATA_HOME", &data),
                ("XDG_CACHE_HOME", &cache),
                ("XDG_STATE_HOME", &state),
            ]),
            home.path(),
        );
        assert_eq!(
            paths,
            Paths {
                config: under("c/nota"),
                data: under("d/nota"),
                cache: under("k/nota"),
                state: under("s/nota"),
            }
        );
    }

    #[test]
    fn ignora_rutas_relativas_o_vacias_como_pide_la_especificacion() {
        let home = tempfile::tempdir().unwrap();
        let paths = Paths::from_xdg(
            env(&[("XDG_CONFIG_HOME", "relativa"), ("XDG_CACHE_HOME", "")]),
            home.path(),
        );
        assert_eq!(paths.config, home.path().join(".config/nota"));
        assert_eq!(paths.cache, home.path().join(".cache/nota"));
    }
}
