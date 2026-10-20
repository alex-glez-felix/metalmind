//! El tooltip de un elemento: un texto en una caja como la de los menús.

use gpui_kit::{
    AnyView, App, AppContext as _, Context, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div, relative,
};

use crate::theme::Theme;

pub struct Tooltip {
    text: SharedString,
    /// Un error: el texto va en `ui.error`.
    error: bool,
}

impl Tooltip {
    /// Para `tooltip` de GPUI, que pide la vista cada vez que se muestra.
    pub fn view(text: SharedString, cx: &mut App) -> AnyView {
        cx.new(|_| Self { text, error: false }).into()
    }

    /// Como [`Tooltip::view`], con el texto en el color de error.
    pub fn error_view(text: SharedString, cx: &mut App) -> AnyView {
        cx.new(|_| Self { text, error: true }).into()
    }
}

impl Render for Tooltip {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        // Se pinta fuera del árbol de la ventana: no hereda la fuente.
        div()
            .font_family(theme.font_family.clone())
            .text_size(theme.ui_font_size)
            .line_height(relative(theme.text.ui_md.line_height))
            .px(theme.layout.tooltip_padding_x)
            .py(theme.layout.tooltip_padding_y)
            .bg(theme.ui.menu_background)
            .border(theme.stroke.border)
            .border_color(theme.ui.menu_border)
            .rounded(theme.radius.md)
            .text_color(if self.error {
                theme.ui.error
            } else {
                theme.ui.foreground
            })
            .child(self.text.clone())
    }
}
