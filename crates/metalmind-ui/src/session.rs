//! Lo que Metalmind recuerda entre sesiones, en el directorio de estado (06 §8):
//! el último vault abierto y, de cada vault, cómo quedó el panel lateral.
//! Nada de esto es del usuario: si falta o está roto, se empieza de cero.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use metalmind_config::Paths;
use metalmind_vault::{FileError, VaultPath};
use serde::{Deserialize, Serialize};

use crate::panel::PanelView;

const VAULT_STATE: &str = "ui.yaml";

#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("no se pudo crear {}: {source}", path.display())]
    CreateDir { path: PathBuf, source: io::Error },
    #[error(transparent)]
    Write(#[from] FileError),
    #[error("no se pudo escribir el estado: {0}")]
    Serialize(#[from] serde_saphyr::SerializeError),
    #[error("la ruta {} no es UTF-8", .0.display())]
    NotUtf8(PathBuf),
}

/// El estado de la UI de un vault.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct VaultState {
    pub panel: PanelState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PanelState {
    pub visible: bool,
    pub view: SavedView,
    /// Las carpetas abiertas del árbol, relativas al vault.
    pub expanded: Vec<String>,
}

/// [`PanelView`] como se escribe.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SavedView {
    #[default]
    Folders,
    Tags,
}

impl Default for PanelState {
    fn default() -> Self {
        Self {
            visible: true,
            view: SavedView::default(),
            expanded: Vec::new(),
        }
    }
}

impl PanelState {
    #[must_use]
    pub fn view(&self) -> PanelView {
        match self.view {
            SavedView::Folders => PanelView::Folders,
            SavedView::Tags => PanelView::Tags,
        }
    }

    /// Las carpetas guardadas que todavía son rutas válidas.
    pub fn expanded(&self) -> impl Iterator<Item = VaultPath> + '_ {
        self.expanded
            .iter()
            .filter_map(|path| VaultPath::new(path).ok())
    }

    #[must_use]
    pub fn new<'a>(
        visible: bool,
        view: PanelView,
        expanded: impl Iterator<Item = &'a VaultPath>,
    ) -> Self {
        Self {
            visible,
            view: match view {
                PanelView::Folders => SavedView::Folders,
                PanelView::Tags => SavedView::Tags,
            },
            expanded: expanded.map(|path| path.as_str().to_owned()).collect(),
        }
    }
}

impl VaultState {
    /// El estado guardado del vault en `root`; el de por defecto si no hay
    /// o no se puede leer, con un aviso en el log.
    #[must_use]
    pub fn load(paths: &Paths, root: &Path) -> Self {
        let path = paths.vault_state(root).join(VAULT_STATE);
        match metalmind_vault::read(&path) {
            Ok(Some(text)) => serde_saphyr::from_str(&text).unwrap_or_else(|error| {
                log::warn!("se ignora {}: {error}", path.display());
                Self::default()
            }),
            Ok(None) => Self::default(),
            Err(error) => {
                log::warn!("{error}");
                Self::default()
            }
        }
    }

    /// # Errors
    /// Si no se puede crear la carpeta o escribir el archivo.
    pub fn save(&self, paths: &Paths, root: &Path) -> Result<(), StateError> {
        let text = serde_saphyr::to_string(self)?;
        write(&paths.vault_state(root).join(VAULT_STATE), &text)
    }
}

/// El último vault que se abrió, si se anotó alguno.
#[must_use]
pub fn last_vault(paths: &Paths) -> Option<PathBuf> {
    let file = paths.last_vault_file();
    match metalmind_vault::read(&file) {
        Ok(text) => text
            .map(|text| PathBuf::from(text.trim_end_matches('\n')))
            .filter(|path| path.is_absolute()),
        Err(error) => {
            log::warn!("{error}");
            None
        }
    }
}

/// Anota `root` como el último vault abierto.
///
/// # Errors
/// Si `root` no es UTF-8 o no se puede escribir.
pub fn remember_vault(paths: &Paths, root: &Path) -> Result<(), StateError> {
    let text = root
        .to_str()
        .ok_or_else(|| StateError::NotUtf8(root.to_owned()))?;
    write(&paths.last_vault_file(), &format!("{text}\n"))
}

fn write(file: &Path, text: &str) -> Result<(), StateError> {
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|source| StateError::CreateDir {
            path: dir.to_owned(),
            source,
        })?;
    }
    metalmind_vault::write_atomic(file, |out| out.write_all(text.as_bytes()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn sin_estado_el_panel_se_ve_en_carpetas() {
        let dir = TempDir::new().unwrap();
        let paths = Paths::under(dir.path());
        let state = VaultState::load(&paths, Path::new("/notas"));
        assert_eq!(state, VaultState::default());
        assert!(state.panel.visible);
        assert_eq!(state.panel.view(), PanelView::Folders);
        assert_eq!(last_vault(&paths), None);
    }

    #[test]
    fn cada_vault_recuerda_lo_suyo() {
        let dir = TempDir::new().unwrap();
        let paths = Paths::under(dir.path());
        let diario = VaultPath::new("diario").unwrap();
        let state = VaultState {
            panel: PanelState::new(false, PanelView::Tags, [&diario].into_iter()),
        };
        state.save(&paths, Path::new("/notas")).unwrap();
        assert_eq!(VaultState::load(&paths, Path::new("/notas")), state);
        assert_eq!(
            VaultState::load(&paths, Path::new("/otras")),
            VaultState::default()
        );
        let loaded = VaultState::load(&paths, Path::new("/notas"));
        assert_eq!(loaded.panel.expanded().collect::<Vec<_>>(), [diario]);
    }

    #[test]
    fn un_estado_roto_se_ignora() {
        let dir = TempDir::new().unwrap();
        let paths = Paths::under(dir.path());
        let state_dir = paths.vault_state(Path::new("/notas"));
        fs::create_dir_all(&state_dir).unwrap();
        let text = "panel:\n  visible: [\n  expanded: [\".oculta\", \"ok\"]\n";
        fs::write(state_dir.join(VAULT_STATE), text).unwrap();
        assert_eq!(
            VaultState::load(&paths, Path::new("/notas")),
            VaultState::default()
        );
        let text = "panel:\n  expanded: [\".oculta\", \"ok\"]\n";
        fs::write(state_dir.join(VAULT_STATE), text).unwrap();
        let loaded = VaultState::load(&paths, Path::new("/notas"));
        let expanded: Vec<String> = loaded
            .panel
            .expanded()
            .map(|path| path.as_str().to_owned())
            .collect();
        assert_eq!(expanded, ["ok"]);
    }

    #[test]
    fn recuerda_el_ultimo_vault() {
        let dir = TempDir::new().unwrap();
        let paths = Paths::under(dir.path());
        remember_vault(&paths, Path::new("/notas/mías")).unwrap();
        assert_eq!(last_vault(&paths), Some(PathBuf::from("/notas/mías")));
        remember_vault(&paths, Path::new("/otras")).unwrap();
        assert_eq!(last_vault(&paths), Some(PathBuf::from("/otras")));
    }
}
