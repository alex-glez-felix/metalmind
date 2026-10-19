//! Los tokens de la barra de estado de cada panel.

use gpui_kit::{Hsla, Pixels, px};
use metalmind_config as config;

use super::hsla;

#[derive(Clone, Debug)]
pub struct StatusBar {
    pub background: Hsla,
    pub foreground: Hsla,
    /// La barra de un panel sin foco.
    pub unfocused_foreground: Hsla,
    pub border: Hsla,
    /// El indicador del autoguardado: "guardado", "sin guardar" y "cambió en
    /// disco".
    pub saved: Hsla,
    pub unsaved: Hsla,
    pub changed_on_disk: Hsla,
    pub padding_x: Pixels,
    pub padding_y: Pixels,
    pub gap: Pixels,
    /// Cuánto cede el nombre de la nota frente al mensaje (`flex_shrink`).
    pub file_shrink: f32,
}

impl StatusBar {
    pub(super) fn new(ui: &config::UiColors, layout: &config::Layout) -> Self {
        Self {
            background: hsla(ui.status_background),
            foreground: hsla(ui.status_foreground),
            unfocused_foreground: hsla(ui.status_unfocused_foreground),
            border: hsla(ui.status_border),
            saved: hsla(ui.status_saved),
            unsaved: hsla(ui.status_unsaved),
            changed_on_disk: hsla(ui.status_changed_on_disk),
            padding_x: px(layout.status_padding_x.0),
            padding_y: px(layout.status_padding_y.0),
            gap: px(layout.status_gap.0),
            file_shrink: layout.status_file_shrink.0,
        }
    }
}
