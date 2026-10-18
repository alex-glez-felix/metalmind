//! `config.yaml`: lo que el usuario ajusta. Todo tiene default, así que un
//! archivo ausente o vacío es una config válida.

use std::path::{Path, PathBuf};

use crate::diagnostic::{Loaded, Location};
use crate::history::HistoryRetention;
use crate::parse::{Spanned, parse};
use crate::theme::DEFAULT_THEME;
use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Nombre del tema: uno de `<config>/themes/*.yaml` o uno embebido.
    theme: Spanned<String>,
    /// Vault que se abre si no se pasa ninguno; `~/` se expande.
    vault: Option<PathBuf>,
    pub font: Font,
    pub editor: Editor,
    pub panes: Panes,
    pub panel: Panel,
    pub search: Search,
    pub history: HistoryRetention,
    /// Carpeta de las plantillas, relativa al vault.
    pub templates_folder: String,
    pub daily: Daily,
}

/// La nota del día. Apagada por defecto: el flujo de captura es el buzón.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Daily {
    pub enabled: bool,
    /// Carpeta de las notas del día, relativa al vault; vacía es la raíz.
    pub folder: String,
    /// El nombre de la nota, con los códigos de fecha de `{{date:…}}`; un
    /// `/` la mete en subcarpetas.
    pub format: String,
    /// Nombre de una plantilla de `templates_folder`; sin ella, la nota nace vacía.
    pub template: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Panes {
    /// Ancho de la línea entre dos paneles, en píxeles lógicos.
    pub border_width: u8,
}

/// El panel lateral con el árbol de carpetas.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Panel {
    pub width: PanelWidth,
}

/// Ancho del panel lateral, en píxeles lógicos.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(try_from = "u16")]
pub struct PanelWidth(u16);

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default)]
pub struct Font {
    /// `None`: la primera monoespaciada instalada de una lista conocida.
    pub family: Option<String>,
    pub size: FontSize,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(try_from = "f32")]
pub struct FontSize(f32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Editor {
    /// El número de cada línea en el margen izquierdo.
    pub number: bool,
    /// La distancia de cada línea a la del cursor; con `number`, la del
    /// cursor muestra su número (el modo híbrido de vim).
    pub relativenumber: bool,
    /// Líneas que se dejan visibles por encima y por debajo del cursor.
    pub scrolloff: u32,
    pub tabstop: TabStop,
    /// `Tab` inserta espacios.
    pub expandtab: bool,
    pub padding: Padding,
}

/// Espacio entre el borde del panel y el texto, en píxeles lógicos.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Padding {
    /// A la izquierda, antes de los números de línea, y a la derecha.
    pub horizontal: u8,
    /// Arriba y abajo.
    pub vertical: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(try_from = "u8")]
pub struct TabStop(u8);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Search {
    pub ignorecase: bool,
    /// Con `ignorecase`, una mayúscula en el patrón vuelve la búsqueda exacta.
    pub smartcase: bool,
}

impl Settings {
    /// Lee `config.yaml`. Nunca falla: con un error se usan los defaults y el
    /// error va en los diagnósticos. Es también la recarga en caliente.
    #[must_use]
    pub fn parse(text: &str) -> Loaded<Self> {
        match parse(text) {
            Ok((value, diagnostics)) => Loaded { value, diagnostics },
            Err(error) => Loaded {
                value: Self::default(),
                diagnostics: vec![error],
            },
        }
    }

    #[must_use]
    pub fn theme(&self) -> &str {
        self.theme.get_ref()
    }

    /// Dónde se escribió el valor de `theme`, si salió de un archivo.
    pub(crate) fn theme_location(&self) -> Option<Location> {
        self.theme.location()
    }

    /// El vault por defecto, con `~/` resuelto contra `home`.
    #[must_use]
    pub fn vault(&self, home: &Path) -> Option<PathBuf> {
        let vault = self.vault.as_deref()?;
        Some(match vault.strip_prefix("~") {
            Ok(rest) => home.join(rest),
            Err(_) => vault.to_owned(),
        })
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Spanned::new(DEFAULT_THEME.to_owned()),
            vault: None,
            font: Font::default(),
            editor: Editor::default(),
            panes: Panes::default(),
            panel: Panel::default(),
            search: Search::default(),
            history: HistoryRetention::default(),
            templates_folder: "plantillas".to_owned(),
            daily: Daily::default(),
        }
    }
}

impl Default for Daily {
    fn default() -> Self {
        Self {
            enabled: false,
            folder: "diario".to_owned(),
            format: "YYYY-MM-DD".to_owned(),
            template: None,
        }
    }
}

impl Default for Font {
    fn default() -> Self {
        Self {
            family: None,
            size: FontSize(15.),
        }
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            number: true,
            relativenumber: true,
            scrolloff: 0,
            tabstop: TabStop(4),
            expandtab: true,
            padding: Padding::default(),
        }
    }
}

impl Default for Panes {
    fn default() -> Self {
        Self { border_width: 2 }
    }
}

impl Default for Panel {
    fn default() -> Self {
        Self {
            width: PanelWidth(260),
        }
    }
}

impl Default for Padding {
    fn default() -> Self {
        Self {
            horizontal: 12,
            vertical: 8,
        }
    }
}

impl FontSize {
    const MIN: f32 = 4.;
    const MAX: f32 = 200.;

    #[must_use]
    pub fn get(self) -> f32 {
        self.0
    }
}

impl TryFrom<f32> for FontSize {
    type Error = String;

    fn try_from(size: f32) -> Result<Self, String> {
        if (Self::MIN..=Self::MAX).contains(&size) {
            Ok(Self(size))
        } else {
            Err(format!(
                "el tamaño de fuente debe estar entre {} y {}, no {size}",
                Self::MIN,
                Self::MAX
            ))
        }
    }
}

impl PanelWidth {
    const MIN: u16 = 120;
    const MAX: u16 = 1200;

    #[must_use]
    pub fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for PanelWidth {
    type Error = String;

    fn try_from(width: u16) -> Result<Self, String> {
        if (Self::MIN..=Self::MAX).contains(&width) {
            Ok(Self(width))
        } else {
            Err(format!(
                "el ancho del panel debe estar entre {} y {}, no {width}",
                Self::MIN,
                Self::MAX
            ))
        }
    }
}

impl TabStop {
    #[must_use]
    pub fn get(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for TabStop {
    type Error = String;

    fn try_from(width: u8) -> Result<Self, String> {
        if width == 0 {
            Err("tabstop debe ser al menos 1".to_owned())
        } else {
            Ok(Self(width))
        }
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
