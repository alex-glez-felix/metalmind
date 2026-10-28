//! La línea de `:` como pop-up centrado arriba, como la paleta y el finder
//! (#193): lo escrito, que se parte en filas en vez de recortarse, y debajo
//! la lista de su autocompletado (el historial, los nombres de comando o el
//! argumento), que se acepta con su tecla o un clic.

use std::ops::Range;

use gpui_kit::{
    AnchoredPositionMode, AnyElement, Context, Div, HighlightStyle, InteractiveElement as _,
    IntoElement, ParentElement as _, Pixels, SharedString, Stateful, Styled, StyledText,
    UnderlineStyle, anchored, canvas, deferred, div, fill, point, prelude::FluentBuilder as _,
    relative, size,
};

use super::Editor;
use super::completion_menu::MenuRow;
use super::status_bar::CommandLine;
use crate::theme::Theme;

/// La lista bajo lo escrito: `rows` son las filas `visible`.
pub(super) struct PopupList {
    pub rows: Vec<MenuRow>,
    pub visible: Range<usize>,
    pub selected: usize,
    pub accept_keys: Option<String>,
}

impl Editor {
    /// El pop-up de `line` sobre todo lo demás, centrado arriba de la
    /// ventana de ancho `window_width` como los pickers: la franja que lo
    /// centra no atrapa el ratón.
    pub(super) fn command_popup(
        &mut self,
        line: CommandLine,
        window_width: Pixels,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let list = self.command_line_list(theme, cx);
        let popup = div()
            .debug_selector(|| "command-popup".to_owned())
            .occlude()
            .w(theme.layout.picker_width)
            .max_w(relative(0.9))
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(theme.ui.picker_background)
            .border(theme.stroke.border)
            .border_color(theme.ui.picker_border)
            .rounded(theme.radius.lg)
            .map(|element| theme.ui_text(element))
            .child(input(line, theme))
            .children(list.map(|list| {
                div()
                    .border_t(theme.stroke.border)
                    .border_color(theme.ui.picker_border)
                    .children(rows(list, theme, cx))
            }));
        let band = div()
            .w(window_width)
            .pt(theme.layout.picker_top)
            .flex()
            .justify_center()
            .child(popup);
        deferred(
            anchored()
                .position_mode(AnchoredPositionMode::Window)
                .position(point(Pixels::ZERO, Pixels::ZERO))
                .child(band),
        )
        .with_priority(1)
        .into_any_element()
    }
}

/// El símbolo, tenue, y lo escrito con el preedit subrayado y el cursor al
/// final: si no cabe, sigue en la fila de abajo.
fn input(line: CommandLine, theme: &Theme) -> Div {
    let CommandLine {
        symbol,
        typed: mut text,
        preedit,
    } = line;
    let typed_len = text.len();
    text.push_str(&preedit);
    let end = text.len();
    let underline = HighlightStyle {
        underline: Some(UnderlineStyle {
            thickness: theme.stroke.text_line,
            ..UnderlineStyle::default()
        }),
        ..HighlightStyle::default()
    };
    let styled = StyledText::new(SharedString::from(text))
        .with_highlights((typed_len < end).then_some((typed_len..end, underline)));
    let layout = styled.layout().clone();
    let (caret_width, caret_color) = (theme.stroke.caret, theme.cursor.insert);
    div()
        .debug_selector(|| "command-popup-input".to_owned())
        .flex()
        .items_start()
        .gap(theme.layout.picker_gap)
        .px(theme.layout.picker_padding_x)
        .py(theme.layout.picker_input_padding_y)
        .child(
            div()
                .flex_none()
                .text_color(theme.ui.picker_muted)
                .child(symbol.to_string()),
        )
        .child(
            div()
                .relative()
                .flex_1()
                .min_w_0()
                .text_color(theme.ui.foreground)
                .child(styled)
                .child(
                    canvas(
                        |_, _, _| (),
                        move |bounds, (), window, _| {
                            let origin = layout.position_for_index(end).unwrap_or(bounds.origin);
                            let caret = size(caret_width, layout.line_height());
                            window.paint_quad(fill(
                                gpui_kit::Bounds::new(origin, caret),
                                caret_color,
                            ));
                        },
                    )
                    .absolute()
                    .size_full(),
                ),
        )
}

/// Como las del picker, con el detalle contra el borde derecho, donde la
/// paleta pone el atajo; en la elegida, la tecla que la acepta.
fn rows(list: PopupList, theme: &Theme, cx: &mut Context<Editor>) -> Vec<Stateful<Div>> {
    let PopupList {
        rows,
        visible,
        selected,
        accept_keys,
    } = list;
    visible
        .zip(rows)
        .map(|(row, shown)| {
            let element = popup_row(row, shown, row == selected, accept_keys.clone(), theme);
            Editor::accept_on_click(row, element, cx)
        })
        .collect()
}

fn popup_row(
    row: usize,
    shown: MenuRow,
    selected: bool,
    accept_keys: Option<String>,
    theme: &Theme,
) -> Stateful<Div> {
    let hover = theme.ui.picker_hover;
    let muted = |text: String| {
        div()
            .map(|element| theme.ui_small_text(element))
            .text_color(theme.ui.picker_muted)
            .child(text)
    };
    div()
        .id(("command-popup-row", row))
        .debug_selector(move || format!("command-popup-row-{row}"))
        .h(theme.picker_row_height())
        .px(theme.layout.picker_padding_x)
        .flex()
        .items_center()
        .gap(theme.layout.picker_gap)
        .when(selected, |element| element.bg(theme.ui.picker_selected))
        .when(!selected, |element| {
            element.hover(move |style| style.bg(hover))
        })
        .child(shown.icon)
        .child(
            div()
                .min_w_0()
                .truncate()
                .when(shown.detail.is_none(), Styled::flex_1)
                .child(shown.name),
        )
        .children(
            shown
                .detail
                .map(|detail| muted(detail).flex_1().min_w_0().truncate().text_right()),
        )
        .children(shown.count.map(|count| muted(count).flex_none()))
        // En todas las filas y solo se ve en la elegida: así la lista le
        // guarda su columna.
        .children(accept_keys.map(|keys| {
            muted(keys).flex_none().when(!selected, |keys| {
                keys.text_color(gpui_kit::transparent_black())
            })
        }))
}
