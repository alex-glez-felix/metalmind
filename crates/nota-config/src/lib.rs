//! Configuración de `nota`: rutas XDG, `config.toml` y temas. Sin GUI: la UI
//! convierte lo que sale de aquí a sus propios tipos.
//!
//! Nada de esto impide arrancar: un archivo roto se reemplaza por los
//! defaults y el problema vuelve como [`Diagnostic`].

mod config;
mod diagnostic;
mod parse;
mod paths;
mod settings;
mod theme;

pub use config::Config;
pub use diagnostic::{Diagnostic, Loaded, Location, Severity};
pub use paths::{NoHomeDir, Paths};
pub use settings::{Editor, Font, FontSize, Search, Settings, TabStop};
pub use theme::{
    BUILTIN_THEMES, Color, CursorColors, DEFAULT_THEME, SyntaxColors, Theme, Themes, UiColors,
};
