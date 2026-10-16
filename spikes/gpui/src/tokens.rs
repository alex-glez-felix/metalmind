use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{App, Hsla, Pixels, SharedString, px};

/// Los tokens que usa el spike, sacados del tema de gpui-component.
#[derive(Clone, Debug)]
pub struct Tokens {
    pub background: Hsla,
    pub foreground: Hsla,
    pub muted: Hsla,
    pub muted_foreground: Hsla,
    pub border: Hsla,
    pub caret: Hsla,
    pub block_cursor: Hsla,
    pub selection: Hsla,
    pub danger: Hsla,
    pub mono_font_family: SharedString,
    pub mono_font_size: Pixels,
    pub caret_width: Pixels,
    pub underline_thickness: Pixels,
}

/// gpui-component no expone estos dos; son los valores de su editor en Linux.
const CARET_WIDTH: Pixels = px(2.);
const UNDERLINE_THICKNESS: Pixels = px(1.);
/// El cursor de bloque deja ver el carácter que tapa.
const BLOCK_CURSOR_ALPHA: f32 = 0.5;

impl Tokens {
    pub fn from_app(cx: &App) -> Self {
        let theme = cx.theme();
        Self {
            background: theme.background,
            foreground: theme.foreground,
            muted: theme.muted,
            muted_foreground: theme.muted_foreground,
            border: theme.border,
            caret: theme.caret,
            block_cursor: theme.caret.opacity(BLOCK_CURSOR_ALPHA),
            selection: theme.selection,
            danger: theme.danger,
            mono_font_family: theme.mono_font_family.clone(),
            mono_font_size: theme.mono_font_size,
            caret_width: CARET_WIDTH,
            underline_thickness: UNDERLINE_THICKNESS,
        }
    }
}
