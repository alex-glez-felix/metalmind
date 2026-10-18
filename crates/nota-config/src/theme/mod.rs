//! Temas: una paleta con nombres y los colores semánticos que la UI usa.
//! Hay temas embebidos; los del usuario van en `<config>/themes/*.toml` y
//! pueden extender a otro con `extends`.

mod color;
mod groups;
mod loader;

use std::collections::BTreeMap;

use serde::Deserialize;
use toml::Spanned;

pub use color::Color;
pub use groups::{CursorColors, SyntaxColors, UiColors};
pub use loader::{LoadError, Themes};

use color::Hex;
use groups::{RawCursor, RawSyntax, RawUi};

/// El tema si `config.toml` no dice otro, y el de respaldo si el elegido falla.
pub const DEFAULT_THEME: &str = "nota-oscuro";

/// Los temas que vienen con nota.
pub const BUILTIN_THEMES: &[(&str, &str)] = &[
    ("nota-oscuro", include_str!("../../themes/nota-oscuro.toml")),
    ("nota-claro", include_str!("../../themes/nota-claro.toml")),
];

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub name: String,
    pub ui: UiColors,
    pub cursor: CursorColors,
    pub syntax: SyntaxColors,
}

/// Un archivo de tema tal como se escribe.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ThemeFile {
    extends: Option<Spanned<String>>,
    palette: BTreeMap<String, Hex>,
    ui: RawUi,
    cursor: RawCursor,
    syntax: RawSyntax,
}
