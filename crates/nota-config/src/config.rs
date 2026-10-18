//! Todo lo que se lee al arrancar.

use std::io;
use std::path::Path;

use crate::diagnostic::{Diagnostic, Loaded, Location};
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

    /// Lee `config.toml` y el tema que elige. Sin archivo valen los
    /// defaults; con uno inválido también, y el error vuelve en los
    /// diagnósticos. Un tema que no carga se cambia por [`DEFAULT_THEME`].
    ///
    /// # Errors
    /// Solo si el tema embebido por defecto no carga, lo que es un bug.
    pub fn load(paths: &Paths) -> Result<Loaded<Self>, Diagnostic> {
        let file = paths.config_file();
        let text = read(&file);
        let Loaded {
            value: settings,
            mut diagnostics,
        } = match &text {
            Ok(Some(text)) => Settings::parse(text),
            Ok(None) | Err(_) => Loaded {
                value: Settings::default(),
                diagnostics: Vec::new(),
            },
        };
        if let Err(error) = &text {
            diagnostics.push(error.clone());
        }
        for diagnostic in &mut diagnostics {
            diagnostic.file = Some(file.clone());
        }
        let theme = match Themes::new(paths.themes_dir()).load(settings.theme()) {
            Ok(loaded) => loaded,
            Err(LoadError { error, warnings }) => {
                diagnostics.extend(warnings);
                diagnostics.push(match (&error.file, &text) {
                    // El tema elegido no se encontró: el error está en `config.toml`.
                    (None, Ok(Some(text))) => error
                        .at(Some(Location::of(text, settings.theme_offset())))
                        .in_file(&file),
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

/// El texto de `file`, `None` si no existe.
fn read(file: &Path) -> Result<Option<String>, Diagnostic> {
    match std::fs::read_to_string(file) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(Diagnostic::error(format!("no se pudo leer: {error}"))),
    }
}

fn builtin_theme() -> Result<Loaded<Theme>, Diagnostic> {
    Themes::builtin()
        .load(DEFAULT_THEME)
        .map_err(|failed| failed.error)
}
