//! Todo lo que se lee al arrancar.

use std::io;

use crate::diagnostic::{Diagnostic, Loaded};
use crate::paths::Paths;
use crate::settings::Settings;
use crate::theme::{DEFAULT_THEME, Theme, Themes};

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub settings: Settings,
    pub theme: Theme,
}

impl Config {
    /// Lee `config.toml` y el tema que elige. Sin archivo valen los
    /// defaults; con uno inválido también, y el error vuelve en los
    /// diagnósticos. Un tema que no carga se cambia por [`DEFAULT_THEME`].
    ///
    /// # Errors
    /// Solo si el tema embebido por defecto no carga, lo que es un bug.
    pub fn load(paths: &Paths) -> Result<Loaded<Self>, Diagnostic> {
        let file = paths.config_file();
        let Loaded {
            value: settings,
            mut diagnostics,
        } = match std::fs::read_to_string(&file) {
            Ok(text) => Settings::parse(&text),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Loaded {
                value: Settings::default(),
                diagnostics: Vec::new(),
            },
            Err(error) => Loaded {
                value: Settings::default(),
                diagnostics: vec![Diagnostic::error(format!("no se pudo leer: {error}"))],
            },
        };
        for diagnostic in &mut diagnostics {
            diagnostic.file = Some(file.clone());
        }
        let theme = match Themes::new(paths.themes_dir()).load(&settings.theme) {
            Ok(loaded) => loaded,
            Err(error) => {
                diagnostics.push(error);
                Themes::builtin().load(DEFAULT_THEME)?
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
