//! Cómo se ve una lista del autocompletado y dónde va: las filas que se
//! ven, cada una con su ícono, su nombre, un detalle tenue (el alias de un
//! tag, la carpeta de una nota) y un conteo, y en la elegida la tecla que
//! la acepta. No pasa de `max_width`: se recorta primero el detalle, luego
//! el nombre, nunca el conteo ni la tecla.

use std::ops::Range;

use gpui_kit::{
    Bounds, Context, Div, InteractiveElement as _, ParentElement as _, Pixels, Stateful, Styled,
    div, prelude::FluentBuilder as _,
};

use crate::theme::Theme;

/// Una fila de la lista.
pub(super) struct MenuRow {
    pub icon: Div,
    pub name: String,
    pub detail: Option<String>,
    pub count: Option<String>,
}

/// Lo que se pinta de la lista: `rows` son las filas `visible`; `wire` le
/// pone a cada fila lo que hace un clic.
pub(super) struct CompletionMenu {
    pub rows: Vec<MenuRow>,
    pub visible: Range<usize>,
    pub selected: usize,
    pub accept_keys: Option<String>,
    pub max_width: Pixels,
}

impl CompletionMenu {
    pub(super) fn render<V: 'static>(
        self,
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
        for (row, shown) in self.visible.zip(self.rows) {
            let selected = row == self.selected;
            let keys = selected.then(|| self.accept_keys.clone()).flatten();
            // El conteo, o si no hay, la tecla, van contra el borde derecho.
            let keys_pushed = shown.count.is_none();
            let muted = |part: &'static str, text: String| {
                div()
                    .debug_selector(move || format!("completion-{row}-{part}"))
                    .map(|element| theme.ui_small_text(element))
                    .text_color(theme.ui.panel_muted)
                    .child(text)
            };
            let element = div()
                .id(("completion", row))
                .debug_selector(move || format!("completion-{row}"))
                .h(theme.menu_row_height)
                .px(theme.layout.menu_item_padding_x)
                .flex()
                .items_center()
                .gap(theme.layout.panel_gap)
                .rounded(theme.radius.md)
                .text_color(theme.ui.panel_foreground)
                .when(selected, |element| element.bg(theme.ui.menu_hover))
                .hover(|element| element.bg(theme.ui.menu_hover))
                .child(shown.icon)
                .child(
                    div()
                        .debug_selector(move || format!("completion-{row}-name"))
                        .min_w_0()
                        .truncate()
                        .child(shown.name),
                )
                .children(shown.detail.map(|detail| {
                    // Sin base, toma solo lo que dejan el nombre y el conteo.
                    muted("detail", detail).flex_1().min_w_0().truncate()
                }))
                .children(
                    shown
                        .count
                        .map(|count| muted("count", count).flex_none().ml_auto()),
                )
                .children(keys.map(|keys| {
                    muted("keys", keys)
                        .flex_none()
                        .when(keys_pushed, Styled::ml_auto)
                }));
            menu = menu.child(wire(row, element, cx));
        }
        menu
    }
}

/// Dónde empieza la lista y cuántas filas muestra.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Placement {
    pub y: Pixels,
    pub rows: usize,
}

/// Las medidas de la lista: lo que ocupa una fila y, arriba y abajo, el
/// relleno con el borde.
#[derive(Debug, Clone, Copy)]
pub(super) struct MenuMetrics {
    pub row_height: Pixels,
    pub frame: Pixels,
}

impl MenuMetrics {
    fn height(self, rows: usize) -> Pixels {
        let rows = u16::try_from(rows).unwrap_or(u16::MAX);
        self.row_height * f32::from(rows) + self.frame + self.frame
    }

    /// Cuántas de `wanted` filas caben en `space`.
    fn fitting(self, wanted: usize, space: Pixels) -> usize {
        (1..=wanted)
            .rev()
            .find(|&rows| self.height(rows) <= space)
            .unwrap_or(0)
    }

    /// Bajo `line` si caben las `wanted` filas en `area`, si no encima si ahí
    /// caben; si no cabe en ninguno, del lado con más lugar y solo con las
    /// filas que quepan, al menos una.
    pub(super) fn place(
        self,
        wanted: usize,
        line: Bounds<Pixels>,
        area: Bounds<Pixels>,
    ) -> Placement {
        let below = self.fitting(wanted, area.bottom() - line.bottom());
        let above = self.fitting(wanted, line.top() - area.top());
        let over = |rows: usize| Placement {
            y: line.top() - self.height(rows),
            rows,
        };
        let under = |rows: usize| Placement {
            y: line.bottom(),
            rows,
        };
        if below == wanted {
            under(wanted)
        } else if above == wanted || above > below {
            over(above.max(1))
        } else {
            under(below.max(1))
        }
    }
}

#[cfg(test)]
mod tests;
