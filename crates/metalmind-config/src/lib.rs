//! Configuración de Metalmind: rutas XDG, `config.yaml` y temas, en YAML 1.2.
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
    AutosaveDelay, Daily, Editor, Font, FontSize, Markdown, Padding, Panel, PanelWidth, Panes,
    Search, Settings, TabStop,
};
pub use theme::{
    BUILTIN_THEMES, Color, Count, CursorColors, DEFAULT_THEME, Factor, Fraction, Heading, Headings,
    Layout, Length, LoadError, MarkColumns, Metrics, Px, Radius, RoleColors, Rows, Spacing, Stroke,
    SyntaxColors, Text, TextStyle, Theme, Themes, UiColors, Weight,
};
