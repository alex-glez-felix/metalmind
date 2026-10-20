//! Tokens de tema: todo color y tamaño de la UI sale de aquí.

use gpui_kit::{App, FontWeight, Global, Pixels, SharedString, Styled, px, relative};
use metalmind_config as config;

mod units;

pub use self::units::Gpui;

/// Familias monoespaciadas, en orden de preferencia. GPUI no resuelve alias
/// de fontconfig como `monospace` y entra en pánico con una familia que no
/// existe, así que se elige una instalada antes de pintar.
const MONO_FONTS: &[&str] = &[
    "JetBrainsMono Nerd Font",
    "JetBrains Mono",
    "Lilex",
    "DejaVu Sans Mono",
    "Noto Sans Mono",
    "Liberation Mono",
    "Ubuntu Mono",
];

#[derive(Debug, thiserror::Error)]
#[error("no hay ninguna fuente monoespaciada instalada; se buscó: {}", MONO_FONTS.join(", "))]
pub struct NoMonoFont;

/// Los grupos del tema ya en los tipos de GPUI (`theme.syntax.heading`,
/// `theme.layout.picker_gap`), y a mano solo lo que se calcula con la fuente
/// o sale de `config.yaml`.
#[derive(Clone, Debug)]
pub struct Theme {
    pub ui: config::UiColors<Gpui>,
    pub cursor: config::CursorColors<Gpui>,
    pub syntax: config::SyntaxColors<Gpui>,
    pub radius: config::Radius<Gpui>,
    pub stroke: config::Stroke<Gpui>,
    /// Los tamaños, en veces `font.size`; ya multiplicados son
    /// [`editor_font_size`](Self::editor_font_size) y los de la interfaz.
    pub text: config::Text<Gpui>,
    pub rows: config::Rows<Gpui>,
    pub layout: config::Layout<Gpui>,
    /// Cada nivel, del `#` al `######`: veces la fuente del editor y peso.
    pub headings: [(f32, FontWeight); 6],
    pub font_family: SharedString,
    /// El texto de las notas (`text.editor`).
    pub editor_font_size: Pixels,
    /// El texto de la interfaz (`text.ui_md`).
    pub ui_font_size: Pixels,
    /// Lo secundario de la interfaz (`text.ui_sm`): atajos, detalles y contadores.
    pub ui_small_font_size: Pixels,
    pub pane_border_width: Pixels,
    /// Entre el borde del panel y el texto, con los números de línea.
    pub text_padding_x: Pixels,
    pub text_padding_y: Pixels,
    pub panel_width: Pixels,
    /// Alto de cada fila del árbol: todas iguales, para virtualizarlo.
    pub panel_row_height: Pixels,
    /// Una entrada del menú; con el texto secundario más grande, mide por él.
    pub menu_row_height: Pixels,
    /// Los íconos del panel: `layout.icon_size` veces la fuente, redondeado.
    pub icon_size: Pixels,
}

impl Global for Theme {}

impl Theme {
    /// Los colores y tamaños de `config`; si la familia de su fuente no
    /// está instalada, la primera de [`MONO_FONTS`] que sí.
    ///
    /// # Errors
    /// [`NoMonoFont`] si no hay ninguna.
    pub fn new(config: &config::Config, cx: &App) -> Result<Self, NoMonoFont> {
        let installed = cx.text_system().all_font_names();
        let font_family = pick_font(config.settings.font.family.as_deref(), &installed)?;
        Ok(Self::with_font(config, font_family))
    }

    pub(crate) fn with_font(config: &config::Config, font_family: SharedString) -> Self {
        let config::Theme {
            ui,
            cursor,
            syntax,
            headings,
            metrics,
            ..
        } = &config.theme;
        let config::Metrics {
            radius,
            stroke,
            text,
            rows,
            layout,
            ..
        } = metrics;
        let settings = &config.settings;
        let padding = settings.editor.padding;
        let font_size = settings.font.size.get();
        let ui_font = font_size * text.ui_md.size;
        let ui_small_font = font_size * text.ui_sm.size;
        Self {
            ui: ui.convert(),
            cursor: cursor.convert(),
            syntax: syntax.convert(),
            radius: radius.convert(),
            stroke: stroke.convert(),
            text: text.convert(),
            rows: rows.convert(),
            layout: layout.convert(),
            headings: headings
                .0
                .map(|heading| (heading.size, FontWeight(f32::from(heading.weight)))),
            font_family,
            editor_font_size: px(font_size * text.editor.size),
            ui_font_size: px(ui_font),
            ui_small_font_size: px(ui_small_font),
            pane_border_width: px(f32::from(settings.panes.border_width)),
            text_padding_x: px(f32::from(padding.horizontal)),
            text_padding_y: px(f32::from(padding.vertical)),
            panel_width: px(f32::from(settings.panel.width.get())),
            panel_row_height: px((ui_font * rows.tree.0).round()),
            menu_row_height: px((ui_font.max(ui_small_font) * rows.menu.0).round()),
            icon_size: px((ui_font * layout.icon_size.0).round()),
        }
    }

    #[must_use]
    pub fn get(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    /// `element` con la letra de las notas.
    pub fn editor_text<E: Styled>(&self, element: E) -> E {
        element
            .text_size(self.editor_font_size)
            .line_height(relative(self.text.editor.line_height))
    }

    /// `element` con la letra de la interfaz.
    pub fn ui_text<E: Styled>(&self, element: E) -> E {
        element
            .text_size(self.ui_font_size)
            .line_height(relative(self.text.ui_md.line_height))
    }

    /// `element` con la letra de lo secundario: atajos, detalles y contadores.
    pub fn ui_small_text<E: Styled>(&self, element: E) -> E {
        element
            .text_size(self.ui_small_font_size)
            .line_height(relative(self.text.ui_sm.line_height))
    }

    /// Alto de una línea de texto de la interfaz.
    #[must_use]
    pub fn ui_line(&self) -> Pixels {
        self.ui_font_size * self.text.ui_md.line_height
    }

    /// Alto de una fila del picker: su línea más alta (el texto o el detalle,
    /// que va en `ui_sm`) con aire arriba y abajo.
    #[must_use]
    pub fn picker_row_height(&self) -> Pixels {
        let detail = self.ui_small_font_size * self.text.ui_sm.line_height;
        self.ui_line().max(detail) + self.rows.picker_padding * 2.
    }

    /// El lado corto de los íconos provisionales del panel, que no son
    /// cuadrados; se van con los de Tabler (#41).
    #[must_use]
    pub fn icon_short_side(&self) -> Pixels {
        self.icon_size * ICON_SHAPE_RATIO
    }
}

/// Proporción de las formas de carpeta y nota mientras no hay íconos (#41).
const ICON_SHAPE_RATIO: f32 = 0.75;

fn pick_font(configured: Option<&str>, installed: &[String]) -> Result<SharedString, NoMonoFont> {
    if let Some(family) = configured {
        if installed.iter().any(|name| name == family) {
            return Ok(SharedString::from(family.to_owned()));
        }
        log::warn!("la fuente `{family}` no está instalada; se usa otra monoespaciada");
    }
    first_installed(MONO_FONTS, installed).ok_or(NoMonoFont)
}

fn first_installed(candidates: &[&str], installed: &[String]) -> Option<SharedString> {
    candidates
        .iter()
        .find(|candidate| installed.iter().any(|name| name == *candidate))
        .map(|&family| SharedString::from(family))
}

#[cfg(test)]
#[path = "theme_tests.rs"]
mod tests;
