//! Tokens de tema: todo color y tamaño de la UI sale de aquí.

use gpui_kit::{App, Global, Hsla, Pixels, SharedString, px, rgb};

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
    pub selection: Hsla,
    pub status_background: Hsla,
    pub status_foreground: Hsla,
    pub status_border: Hsla,
    pub modified: Hsla,
    pub error: Hsla,
    pub font_family: SharedString,
    pub font_size: Pixels,
    /// Alto de línea relativo al tamaño de la fuente.
    pub line_height: f32,
    pub caret_width: Pixels,
    pub underline_thickness: Pixels,
    pub border_width: Pixels,
    pub status_padding_x: Pixels,
    pub status_padding_y: Pixels,
    pub status_gap: Pixels,
}

impl Global for Theme {}

impl Theme {
    /// El tema oscuro por defecto, con la primera fuente de [`MONO_FONTS`]
    /// que esté instalada.
    ///
    /// # Errors
    /// [`NoMonoFont`] si no hay ninguna.
    pub fn dark(cx: &App) -> Result<Self, NoMonoFont> {
        let installed = cx.text_system().all_font_names();
        let font_family = first_installed(MONO_FONTS, &installed).ok_or(NoMonoFont)?;
        Ok(Self {
            background: rgb(0x1c_1f_24).into(),
            foreground: rgb(0xd7_da_e0).into(),
            caret: rgb(0x7a_a2_f7).into(),
            block_cursor: Hsla::from(rgb(0x7a_a2_f7)).opacity(0.5),
            selection: Hsla::from(rgb(0x7a_a2_f7)).opacity(0.3),
            status_background: rgb(0x16_18_1d).into(),
            status_foreground: rgb(0x9a_a0_a6).into(),
            status_border: rgb(0x2a_2e_36).into(),
            modified: rgb(0xe0_af_68).into(),
            error: rgb(0xf7_76_8e).into(),
            font_family,
            font_size: px(15.),
            line_height: 1.4,
            caret_width: px(2.),
            underline_thickness: px(1.),
            border_width: px(1.),
            status_padding_x: px(10.),
            status_padding_y: px(3.),
            status_gap: px(16.),
        })
    }

    #[must_use]
    pub fn get(cx: &App) -> &Self {
        cx.global::<Self>()
    }
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
    fn sin_ninguna_candidata_no_hay_fuente() {
        assert_eq!(
            first_installed(MONO_FONTS, &installed(&["Cantarell"])),
            None
        );
    }
}
