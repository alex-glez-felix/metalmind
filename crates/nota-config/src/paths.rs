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
    /// recuperación, historial de `:` y de búsquedas. Las versiones de las
    /// notas van en `data` (06 §3).
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

    /// La carpeta de datos del vault con raíz `root`, que debe venir
    /// canonicalizada: el vault se identifica por su ruta (06 §8). El nombre
    /// lleva el del vault para quien mire la carpeta y un hash de la ruta
    /// completa para que dos vaults con el mismo nombre no choquen.
    #[must_use]
    pub fn vault_data(&self, root: &Path) -> PathBuf {
        self.data.join("vaults").join(vault_key(root))
    }

    #[must_use]
    pub fn config_file(&self) -> PathBuf {
        self.config.join("config.yaml")
    }

    #[must_use]
    pub fn themes_dir(&self) -> PathBuf {
        self.config.join("themes")
    }

    /// Dónde se anota el último vault abierto.
    #[must_use]
    pub fn last_vault_file(&self) -> PathBuf {
        self.state.join("last-vault")
    }

    /// Dónde va el estado de UI del vault de raíz `root`, como sus notas
    /// recientes: cada vault tiene el suyo (06 §8), por su ruta canónica.
    #[must_use]
    pub fn vault_state(&self, root: &Path) -> PathBuf {
        self.state.join("vaults").join(vault_key(root))
    }
}

/// Cuánto del nombre de la carpeta entra en la clave.
const KEY_NAME_LEN: usize = 40;

/// `<nombre>-<hash>`: el nombre para reconocerla a ojo y el hash de la ruta
/// para que dos vaults con el mismo nombre no choquen.
fn vault_key(root: &Path) -> String {
    let name: String = root
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_default()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .take(KEY_NAME_LEN)
        .collect();
    let name = if name.is_empty() { "vault" } else { &name };
    format!("{name}-{:016x}", fnv1a(root.as_os_str().as_encoded_bytes()))
}

/// FNV-1a de 64 bits: a diferencia de `DefaultHasher`, da lo mismo en
/// cualquier versión de Rust, y la clave tiene que sobrevivir a una
/// actualización.
fn fnv1a(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    bytes.iter().fold(OFFSET, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(PRIME)
    })
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
    fn cada_vault_tiene_su_carpeta_de_estado() {
        let paths = Paths::under(Path::new("/x"));
        assert_eq!(paths.last_vault_file(), Path::new("/x/state/last-vault"));
        let notas = paths.vault_state(Path::new("/a/notas"));
        assert_eq!(notas, Path::new("/x/state/vaults/notas-39ed27d9e0362a9f"));
        assert_eq!(notas, paths.vault_state(Path::new("/a/notas")));
        assert_ne!(notas, paths.vault_state(Path::new("/b/notas")));
        let raro = paths.vault_state(Path::new("/a/mis notas: ñ"));
        assert!(raro.ends_with(format!(
            "mis_notas___-{:016x}",
            fnv1a(b"/a/mis notas: \xc3\xb1")
        )));
        let raiz = paths.vault_state(Path::new("/"));
        assert!(
            raiz.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("vault-")
        );
    }

    #[test]
    fn fnv1a_da_los_valores_de_referencia() {
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a(b"foobar"), 0x8594_4171_f739_67e8);
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

    #[test]
    fn cada_vault_tiene_su_carpeta_de_datos() {
        let paths = Paths::under(Path::new("/datos"));
        let notas = paths.vault_data(Path::new("/a/Mis notas"));
        assert!(notas.starts_with("/datos/data/vaults"));
        let name = notas.file_name().unwrap().to_str().unwrap();
        assert!(name.starts_with("Mis_notas-"), "{name}");
        assert_eq!(notas, paths.vault_data(Path::new("/a/Mis notas")));
        assert_ne!(notas, paths.vault_data(Path::new("/b/Mis notas")));
    }
}
