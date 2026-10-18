//! Configuración de `nota`: rutas XDG, `config.yaml` y temas, en YAML 1.2.
//! Sin GUI: la UI convierte lo que sale de aquí a sus propios tipos.
//!
//! Nada de esto impide arrancar: un archivo roto se reemplaza por los
//! defaults y el problema vuelve como [`Diagnostic`].

mod config;
mod diagnostic;
mod file;
mod history;
mod parse;
mod paths;
mod settings;
mod theme;

pub use config::Config;
pub use diagnostic::{Diagnostic, Loaded, Location, Severity};
pub use history::HistoryRetention;
pub use paths::{NoHomeDir, Paths};
pub use settings::{
    Daily, Editor, Font, FontSize, Padding, Panel, PanelWidth, Panes, Search, Settings, TabStop,
};
pub use theme::{
    BUILTIN_THEMES, Color, CursorColors, DEFAULT_THEME, LoadError, SyntaxColors, Theme, Themes,
    UiColors,
};
