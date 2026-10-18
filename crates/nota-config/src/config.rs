//! Todo lo que se lee al arrancar.

use crate::diagnostic::{Diagnostic, Loaded};
use crate::file::{YamlFile, read_yaml};
use crate::paths::Paths;
use crate::settings::Settings;
use crate::theme::{DEFAULT_THEME, LoadError, Theme, Themes};

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub settings: Settings,
    pub theme: Theme,
}

impl Config {
    /// Todo por defecto, sin leer nada del disco.
    ///
    /// # Errors
    /// Solo si el tema embebido por defecto no carga, lo que es un bug.
    pub fn defaults() -> Result<Self, Diagnostic> {
        Ok(Self {
            settings: Settings::default(),
            theme: builtin_theme()?.value,
        })
    }

    /// Lee `config.yaml` y el tema que elige. Sin archivo
    /// valen los defaults; con uno inválido también, y el error vuelve en los
    /// diagnósticos. Un tema que no carga se cambia por [`DEFAULT_THEME`].
    ///
    /// # Errors
    /// Solo si el tema embebido por defecto no carga, lo que es un bug.
    pub fn load(paths: &Paths) -> Result<Loaded<Self>, Diagnostic> {
        let mut diagnostics = Vec::new();
        let (settings, file) = match read_yaml(&paths.config_file()) {
            Ok(Some(YamlFile { path, text })) => {
                let loaded = Settings::parse(&text);
                diagnostics.extend(
                    loaded
                        .diagnostics
                        .into_iter()
                        .map(|diagnostic| diagnostic.in_file(&path)),
                );
                (loaded.value, Some(path))
            }
            Ok(None) => (Settings::default(), None),
            Err(error) => {
                diagnostics.push(error);
                (Settings::default(), None)
            }
        };
        let theme = match Themes::new(paths.themes_dir()).load(settings.theme()) {
            Ok(loaded) => loaded,
            Err(LoadError { error, warnings }) => {
                diagnostics.extend(warnings);
                diagnostics.push(match (&error.file, &file) {
                    // El tema elegido no se encontró: el error está en la config.
                    (None, Some(file)) => error.at(settings.theme_location()).in_file(file),
                    _ => error,
                });
                builtin_theme()?
            }
        };
        diagnostics.extend(theme.diagnostics);
        Ok(Loaded {
            value: Self {
                settings,
                theme: theme.value,
            },
            diagnostics,
        })
    }
}

fn builtin_theme() -> Result<Loaded<Theme>, Diagnostic> {
    Themes::builtin()
        .load(DEFAULT_THEME)
        .map_err(|failed| failed.error)
}
