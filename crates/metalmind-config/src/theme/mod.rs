//! Temas: una paleta con nombres y los colores semánticos que la UI usa.
//! Hay temas embebidos; los del usuario van en `<config>/themes/*.yaml` y
//! pueden extender a otro con `extends`.

mod color;
mod groups;
mod headings;
mod loader;
mod metrics;
mod represent;
mod units;

use std::collections::BTreeMap;

use serde::Deserialize;

pub use color::Color;
pub use groups::{CursorColors, RoleColors, SyntaxColors, UiColors};
pub use headings::{Heading, Headings};
pub use loader::{LoadError, Themes};
pub use metrics::{
    Count, Factor, Fraction, Layout, Length, MarkColumns, Metrics, Px, Radius, Rows, Spacing,
    Stroke, Text, TextColumns, TextStyle, Weight,
};
pub use represent::{Plain, Represent, Units};

use color::Hex;
use groups::{RawCursor, RawRoles, RawSyntax, RawUi};
use headings::RawHeadings;
use metrics::{RawLayout, RawMetrics, RawRadius, RawRows, RawSpacing, RawStroke, RawText};

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
    pub roles: RoleColors,
    pub ui: UiColors,
    pub cursor: CursorColors,
    pub syntax: SyntaxColors,
    pub headings: Headings,
    pub metrics: Metrics,
}

/// Un archivo de tema tal como se escribe.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ThemeFile {
    extends: Option<Spanned<String>>,
    palette: BTreeMap<String, Hex>,
    roles: RawRoles,
    ui: RawUi,
    cursor: RawCursor,
    syntax: RawSyntax,
    headings: RawHeadings,
    spacing: RawSpacing,
    radius: RawRadius,
    stroke: RawStroke,
    text: RawText,
    rows: RawRows,
    layout: RawLayout,
}

impl ThemeFile {
    fn metrics(&self) -> RawMetrics<'_> {
        RawMetrics {
            spacing: &self.spacing,
            radius: &self.radius,
            stroke: &self.stroke,
            text: &self.text,
            rows: &self.rows,
            layout: &self.layout,
        }
    }
}
