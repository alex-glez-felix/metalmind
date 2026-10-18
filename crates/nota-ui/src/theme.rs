//! Tokens de tema: todo color y tamaño de la UI sale de aquí.

use gpui_kit::{App, Global, Hsla, Pixels, SharedString, px, rgb};
use nota_config as config;

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

#[derive(Clone, Debug)]
pub struct Theme {
    pub background: Hsla,
    pub foreground: Hsla,
    pub caret: Hsla,
    /// El cursor de bloque deja ver el carácter que tapa.
    pub block_cursor: Hsla,
    /// El cursor de bloque en Visual.
    pub visual_cursor: Hsla,
    /// El cursor de un panel sin foco.
    pub unfocused_cursor: Hsla,
    pub selection: Hsla,
    /// Fondo del margen de los números de línea.
    pub gutter: Hsla,
    pub gutter_foreground: Hsla,
    /// El número de la línea del cursor.
    pub gutter_current: Hsla,
    pub status_background: Hsla,
    pub status_foreground: Hsla,
    pub status_unfocused_foreground: Hsla,
    pub status_border: Hsla,
    /// La línea entre dos paneles.
    pub pane_border: Hsla,
    pub modified: Hsla,
    pub error: Hsla,
    pub font_family: SharedString,
    pub font_size: Pixels,
    /// Alto de línea relativo al tamaño de la fuente.
    pub line_height: f32,
    pub caret_width: Pixels,
    pub underline_thickness: Pixels,
    pub border_width: Pixels,
    pub pane_border_width: Pixels,
    /// Entre el borde del panel y el texto, con los números de línea.
    pub text_padding_x: Pixels,
    pub text_padding_y: Pixels,
    pub status_padding_x: Pixels,
    pub status_padding_y: Pixels,
    pub status_gap: Pixels,
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

    fn with_font(config: &config::Config, font_family: SharedString) -> Self {
        let config::Theme { ui, cursor, .. } = &config.theme;
        let settings = &config.settings;
        let padding = settings.editor.padding;
        Self {
            background: hsla(ui.background),
            foreground: hsla(ui.foreground),
            caret: hsla(cursor.insert),
            block_cursor: hsla(cursor.normal),
            visual_cursor: hsla(cursor.visual),
            unfocused_cursor: hsla(cursor.unfocused),
            selection: hsla(ui.selection),
            gutter: hsla(ui.gutter),
            gutter_foreground: hsla(ui.gutter_foreground),
            gutter_current: hsla(ui.gutter_current),
            status_background: hsla(ui.status_background),
            status_foreground: hsla(ui.status_foreground),
            status_unfocused_foreground: hsla(ui.status_unfocused_foreground),
            status_border: hsla(ui.status_border),
            pane_border: hsla(ui.pane_border),
            modified: hsla(ui.modified),
            error: hsla(ui.error),
            font_family,
            font_size: px(settings.font.size.get()),
            line_height: 1.4,
            caret_width: px(2.),
            underline_thickness: px(1.),
            border_width: px(1.),
            pane_border_width: px(f32::from(settings.panes.border_width)),
            text_padding_x: px(f32::from(padding.horizontal)),
            text_padding_y: px(f32::from(padding.vertical)),
            status_padding_x: px(10.),
            status_padding_y: px(3.),
            status_gap: px(16.),
        }
    }

    #[must_use]
    pub fn get(cx: &App) -> &Self {
        cx.global::<Self>()
    }
}

fn hsla(color: config::Color) -> Hsla {
    Hsla::from(rgb(color.rgb)).opacity(color.alpha)
}

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
mod tests {
    use super::*;

    fn installed(names: &[&str]) -> Vec<String> {
        names.iter().map(|&name| name.to_owned()).collect()
    }

    #[test]
    fn elige_la_primera_candidata_instalada() {
        let fonts = installed(&["Cantarell", "Liberation Mono", "DejaVu Sans Mono"]);
        assert_eq!(
            first_installed(MONO_FONTS, &fonts).as_deref(),
            Some("DejaVu Sans Mono")
        );
    }

    #[test]
    fn la_fuente_de_la_config_si_esta_instalada() {
        let fonts = installed(&["Iosevka", "DejaVu Sans Mono"]);
        assert_eq!(pick_font(Some("Iosevka"), &fonts).unwrap(), "Iosevka");
        assert_eq!(
            pick_font(Some("No Existe"), &fonts).unwrap(),
            "DejaVu Sans Mono"
        );
        assert_eq!(pick_font(None, &fonts).unwrap(), "DejaVu Sans Mono");
    }

    /// Los valores que `nota-ui` tenía fijos antes de leer temas, y los de paneles.
    /// El borde entre paneles es `comment`, con contraste 3:1: con `border` no se veía.
    #[test]
    fn el_tema_oscuro_embebido_se_ve_igual_que_antes() {
        let theme = Theme::with_font(&config::Config::defaults().unwrap(), "Mono".into());
        let blue = Hsla::from(rgb(0x7a_a2_f7));
        let expected: [(&str, Hsla, Hsla); 14] = [
            ("background", theme.background, rgb(0x1c_1f_24).into()),
            ("foreground", theme.foreground, rgb(0xd7_da_e0).into()),
            ("caret", theme.caret, rgb(0x7a_a2_f7).into()),
            ("block_cursor", theme.block_cursor, blue.opacity(0.5)),
            (
                "visual_cursor",
                theme.visual_cursor,
                Hsla::from(rgb(0xbb_9a_f7)).opacity(0.5),
            ),
            (
                "unfocused_cursor",
                theme.unfocused_cursor,
                Hsla::from(rgb(0x9a_a0_a6)).opacity(0.35),
            ),
            ("selection", theme.selection, blue.opacity(0.3)),
            (
                "status_background",
                theme.status_background,
                rgb(0x16_18_1d).into(),
            ),
            (
                "status_foreground",
                theme.status_foreground,
                rgb(0x9a_a0_a6).into(),
            ),
            (
                "status_unfocused_foreground",
                theme.status_unfocused_foreground,
                rgb(0x5c_63_70).into(),
            ),
            ("status_border", theme.status_border, rgb(0x2a_2e_36).into()),
            ("pane_border", theme.pane_border, rgb(0x5c_63_70).into()),
            ("modified", theme.modified, rgb(0xe0_af_68).into()),
            ("error", theme.error, rgb(0xf7_76_8e).into()),
        ];
        for (name, actual, before) in expected {
            assert_eq!(actual, before, "{name}");
        }
        assert_eq!(theme.font_size, px(15.));
    }

    #[test]
    fn el_margen_del_texto_sale_de_la_config() {
        let mut config = config::Config::defaults().unwrap();
        config.settings.editor.padding = config::Padding {
            horizontal: 20,
            vertical: 0,
        };
        let theme = Theme::with_font(&config, "Mono".into());
        assert_eq!(
            (theme.text_padding_x, theme.text_padding_y),
            (px(20.), px(0.))
        );
    }

    #[test]
    fn el_ancho_del_borde_entre_paneles_sale_de_la_config() {
        let mut config = config::Config::defaults().unwrap();
        let width =
            |config: &config::Config| Theme::with_font(config, "Mono".into()).pane_border_width;
        assert_eq!(width(&config), px(2.));
        config.settings.panes.border_width = 5;
        assert_eq!(width(&config), px(5.));
    }

    #[test]
    fn sin_ninguna_candidata_no_hay_fuente() {
        assert_eq!(
            first_installed(MONO_FONTS, &installed(&["Cantarell"])),
            None
        );
    }
}
