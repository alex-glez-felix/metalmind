//! El tooltip de un elemento: un texto en una caja como la de los menús.

use gpui_kit::{
    AnyView, App, AppContext as _, Context, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div, relative,
};

use crate::theme::Theme;

pub struct Tooltip {
    text: SharedString,
}

impl Tooltip {
    /// Para `tooltip` de GPUI, que pide la vista cada vez que se muestra.
    pub fn view(text: SharedString, cx: &mut App) -> AnyView {
        cx.new(|_| Self { text }).into()
    }
}

impl Render for Tooltip {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        // Se pinta fuera del árbol de la ventana: no hereda la fuente.
        div()
            .font_family(theme.font_family.clone())
            .text_size(theme.ui_font_size)
            .line_height(relative(theme.ui_line_height))
            .px(theme.tooltip_padding_x)
            .py(theme.tooltip_padding_y)
            .bg(theme.menu_background)
            .border(theme.border_width)
            .border_color(theme.menu_border)
            .rounded(theme.radius_md)
            .text_color(theme.foreground)
            .child(self.text.clone())
    }
}
