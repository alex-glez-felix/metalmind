//! Cómo se ve la lista de `#`: las filas que se ven, cada una con su ícono,
//! su nombre, el alias por el que coincidió y su conteo, y en la elegida la
//! tecla que la acepta. No pasa de `max_width`: se recorta primero el
//! alias, luego el nombre, nunca el conteo ni la tecla.

use std::ops::Range;

use gpui_kit::{
    Context, Div, InteractiveElement as _, ParentElement as _, Pixels, Stateful, Styled as _, div,
    prelude::FluentBuilder as _,
};
use metalmind_vault::tags::TagIcon;

use crate::tag_completion::TagCompletions;
use crate::theme::Theme;

/// Lo que se pinta de la lista; `wire` le pone a cada fila lo que hace un
/// clic.
pub(super) struct CompletionMenu<'a> {
    pub list: &'a TagCompletions,
    pub visible: Range<usize>,
    pub accept_keys: Option<String>,
    pub max_width: Pixels,
}

impl CompletionMenu<'_> {
    pub(super) fn render<V: 'static>(
        &self,
        theme: &Theme,
        cx: &mut Context<V>,
        wire: impl Fn(usize, Stateful<Div>, &mut Context<V>) -> Stateful<Div>,
    ) -> Div {
        let mut menu = div()
            .min_w(theme.layout.menu_width.min(self.max_width))
            .max_w(self.max_width)
            .p(theme.layout.menu_padding)
            .flex()
            .flex_col()
            .bg(theme.ui.menu_background)
            .border(theme.stroke.border)
            .border_color(theme.ui.menu_border)
            .rounded(theme.radius.md)
            .map(|element| theme.ui_text(element));
        let rows = self.list.rows().enumerate();
        for (row, (completion, alias)) in rows.skip(self.visible.start).take(self.visible.len()) {
            let selected = row == self.list.selected();
            let keys = selected.then(|| self.accept_keys.clone()).flatten();
            let muted = |part: &'static str, text: String| {
                div()
                    .debug_selector(move || format!("tag-completion-{row}-{part}"))
                    .map(|element| theme.ui_small_text(element))
                    .text_color(theme.ui.panel_muted)
                    .child(text)
            };
            let element = div()
                .id(("tag-completion", row))
                .debug_selector(move || format!("tag-completion-{row}"))
                .h(theme.menu_row_height)
                .px(theme.layout.menu_item_padding_x)
                .flex()
                .items_center()
                .gap(theme.layout.panel_gap)
                .rounded(theme.radius.md)
                .text_color(theme.ui.panel_foreground)
                .when(selected, |element| element.bg(theme.ui.menu_hover))
                .hover(|element| element.bg(theme.ui.menu_hover))
                .child(icon(completion.icon.as_ref(), theme))
                .child(
                    div()
                        .debug_selector(move || format!("tag-completion-{row}-name"))
                        .min_w_0()
                        .truncate()
                        .child(completion.name.clone()),
                )
                .children(alias.map(|alias| {
                    // Sin base, toma solo lo que dejan el nombre y el conteo.
                    muted("alias", format!("alias {alias}"))
                        .flex_1()
                        .min_w_0()
                        .truncate()
                }))
                .child(
                    muted("count", completion.count.to_string())
                        .flex_none()
                        .ml_auto(),
                )
                .children(keys.map(|keys| muted("keys", keys).flex_none()));
            menu = menu.child(wire(row, element, cx));
        }
        menu
    }
}

/// El emoji de la página del tag o un `#` tenue, como en el panel.
fn icon(icon: Option<&TagIcon>, theme: &Theme) -> Div {
    let slot = div().w(theme.icon_size).flex_none().flex().justify_center();
    match icon {
        Some(TagIcon::Emoji(emoji)) => slot.child(emoji.clone()),
        Some(TagIcon::Set { .. }) | None => slot.text_color(theme.ui.panel_muted).child("#"),
    }
}

#[cfg(test)]
mod tests;
