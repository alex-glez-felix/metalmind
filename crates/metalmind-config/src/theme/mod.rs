//! Temas: una paleta con nombres y los colores semánticos que la UI usa.
//! Hay temas embebidos; los del usuario van en `<config>/themes/*.yaml` y
//! pueden extender a otro con `extends`.

mod color;
mod groups;
mod headings;
mod loader;

use std::collections::BTreeMap;

use serde::Deserialize;

pub use color::Color;
pub use groups::{CursorColors, SyntaxColors, UiColors};
pub use headings::{Heading, Headings};
pub use loader::{LoadError, Themes};

use color::Hex;
use groups::{RawCursor, RawSyntax, RawUi};
use headings::RawHeadings;

use crate::parse::Spanned;

/// El tema si `config.yaml` no dice otro, y el de respaldo si el elegido falla.
pub const DEFAULT_THEME: &str = "metalmind-oscuro";

/// Los temas que vienen con Metalmind.
pub const BUILTIN_THEMES: &[(&str, &str)] = &[
    (
        "metalmind-oscuro",
        include_str!("../../themes/metalmind-oscuro.yaml"),
    ),
    (
        "metalmind-claro",
        include_str!("../../themes/metalmind-claro.yaml"),
    ),
];

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub name: String,
    pub ui: UiColors,
    pub cursor: CursorColors,
    pub syntax: SyntaxColors,
    pub headings: Headings,
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
    headings: RawHeadings,
}
