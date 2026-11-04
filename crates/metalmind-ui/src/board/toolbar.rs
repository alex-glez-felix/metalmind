//! La barra de herramientas, la del zoom y el menú contextual: cada entrada es
//! una acción con nombre y muestra su tecla (design.md, 3).

use gpui_kit::{
    AnyElement, App, Context, Div, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, Pixels, Point as ScreenPoint, StatefulInteractiveElement as _, Styled as _,
    Window, anchored, deferred, div, prelude::FluentBuilder as _,
};
use metalmind_board::Shape;
use metalmind_keymap::{BoardAction, View};

use super::{BoardView, Tool};
use crate::actions::{AppKeymap, Dispatch};
use crate::key_button::key_button;
use crate::theme::Theme;

fn button(
    id: &'static str,
    label: &'static str,
    action: BoardAction,
    theme: &Theme,
    cx: &App,
) -> impl IntoElement {
    key_button(id, label, action, View::Board, theme, cx)
}

/// Un botón de herramienta con el mismo borde siempre, para que elegirla no
/// mueva nada: solo cambia de color.
fn tool_button(
    id: &'static str,
    label: &'static str,
    action: BoardAction,
    active: bool,
    theme: &Theme,
    cx: &App,
) -> impl IntoElement {
    let border = if active {
        theme.ui.board_selection
    } else {
        theme.ui.panel_background
    };
    div()
        .border(theme.stroke.border)
        .border_color(border)
        .rounded(theme.radius.md)
        .child(button(id, label, action, theme, cx))
}

fn bar(theme: &Theme) -> Div {
    div()
        .flex()
        .flex_wrap()
        .gap(theme.layout.panel_gap)
        .bg(theme.ui.panel_background)
        .border(theme.stroke.border)
        .border_color(theme.ui.panel_border)
        .rounded(theme.radius.md)
        .map(|bar| theme.ui_small_text(bar))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
}

impl BoardView {
    /// Arriba a la izquierda, lo que crea; en un split angosto se parte en filas.
    pub(super) fn tools(&self, theme: &Theme, cx: &App) -> impl IntoElement + use<> {
        let margin = theme.layout.board_toolbar_margin;
        let tool = self.tool;
        let is = |shape| tool == Tool::Shape(shape);
        div()
            .absolute()
            .left(margin)
            .right(margin)
            .top(margin)
            .flex()
            .justify_start()
            .child(
                bar(theme)
                    .child(tool_button(
                        "board-select",
                        "elegir",
                        BoardAction::ToolSelect,
                        tool == Tool::Select,
                        theme,
                        cx,
                    ))
                    .child(tool_button(
                        "board-rectangle",
                        "caja",
                        BoardAction::ToolRectangle,
                        is(Shape::Rectangle),
                        theme,
                        cx,
                    ))
                    .child(tool_button(
                        "board-ellipse",
                        "elipse",
                        BoardAction::ToolEllipse,
                        is(Shape::Ellipse),
                        theme,
                        cx,
                    ))
                    .child(tool_button(
                        "board-diamond",
                        "rombo",
                        BoardAction::ToolDiamond,
                        is(Shape::Diamond),
                        theme,
                        cx,
                    ))
                    .child(tool_button(
                        "board-text",
                        "texto",
                        BoardAction::ToolText,
                        tool == Tool::Text,
                        theme,
                        cx,
                    ))
                    .child(tool_button(
                        "board-arrow",
                        "flecha",
                        BoardAction::ToolArrow,
                        tool == Tool::Arrow,
                        theme,
                        cx,
                    )),
            )
    }

    /// Abajo a la derecha, el zoom.
    pub(super) fn zoom_bar(theme: &Theme, cx: &App) -> impl IntoElement + use<> {
        let margin = theme.layout.board_toolbar_margin;
        div()
            .absolute()
            .left(margin)
            .right(margin)
            .bottom(margin)
            .flex()
            .justify_end()
            .child(
                bar(theme)
                    .justify_end()
                    .child(button(
                        "board-zoom-out",
                        "alejar",
                        BoardAction::ZoomOut,
                        theme,
                        cx,
                    ))
                    .child(button(
                        "board-zoom-in",
                        "acercar",
                        BoardAction::ZoomIn,
                        theme,
                        cx,
                    ))
                    .child(button(
                        "board-100",
                        "100 %",
                        BoardAction::ZoomToActualSize,
                        theme,
                        cx,
                    ))
                    .child(button(
                        "board-fit",
                        "ver todo",
                        BoardAction::ZoomToFit,
                        theme,
                        cx,
                    )),
            )
    }

    /// Lo que ofrece el menú según lo que haya elegido.
    fn menu_groups(&self) -> Vec<&'static [(&'static str, BoardAction)]> {
        const CREATE: &[(&str, BoardAction)] = &[
            ("Caja", BoardAction::ToolRectangle),
            ("Elipse", BoardAction::ToolEllipse),
            ("Rombo", BoardAction::ToolDiamond),
            ("Texto suelto", BoardAction::ToolText),
        ];
        const ELEMENT: &[(&str, BoardAction)] = &[
            ("Escribir", BoardAction::EditText),
            ("Unir a un nodo", BoardAction::LinkToNode),
            ("Crear caja unida", BoardAction::ToolArrow),
        ];
        const CLIPBOARD_SELECTED: &[(&str, BoardAction)] =
            &[("Copiar", BoardAction::Copy), ("Pegar", BoardAction::Paste)];
        const CLIPBOARD: &[(&str, BoardAction)] = &[("Pegar", BoardAction::Paste)];
        const HISTORY: &[(&str, BoardAction)] = &[
            ("Deshacer", BoardAction::Undo),
            ("Rehacer", BoardAction::Redo),
        ];
        const DELETE: &[(&str, BoardAction)] = &[("Borrar", BoardAction::Delete)];
        if self.selected.is_empty() {
            vec![CREATE, CLIPBOARD, HISTORY]
        } else {
            vec![ELEMENT, CLIPBOARD_SELECTED, HISTORY, DELETE]
        }
    }

    /// El menú del clic derecho.
    pub(super) fn context_menu(
        &self,
        position: ScreenPoint<Pixels>,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let max_width = window.viewport_size().width;
        let mut list = div()
            .min_w(theme.layout.menu_width.min(max_width))
            .max_w(max_width)
            .p(theme.layout.menu_padding)
            .flex()
            .flex_col()
            .bg(theme.ui.menu_background)
            .border(theme.stroke.border)
            .border_color(theme.ui.menu_border)
            .rounded(theme.radius.md)
            .map(|list| theme.ui_text(list))
            .on_mouse_down_out(cx.listener(|view, _, _, cx| {
                view.menu = None;
                cx.notify();
            }));
        for (group_index, group) in self.menu_groups().into_iter().enumerate() {
            if group_index > 0 {
                list = list.child(
                    div()
                        .my(theme.layout.menu_padding)
                        .h(theme.stroke.border)
                        .bg(theme.ui.menu_border),
                );
            }
            for &(label, action) in group {
                list = list.child(menu_row(label, action, theme, cx));
            }
        }
        deferred(anchored().position(position).snap_to_window().child(list))
            .with_priority(1)
            .into_any_element()
    }
}

fn menu_row(
    label: &'static str,
    action: BoardAction,
    theme: &Theme,
    cx: &mut Context<BoardView>,
) -> impl IntoElement {
    let keys = AppKeymap::get(cx).keys_for(action.into(), |view| view == View::Board);
    let color = if action == BoardAction::Delete {
        theme.ui.error
    } else {
        theme.ui.panel_foreground
    };
    div()
        .id(label)
        .debug_selector(|| label.to_owned())
        .flex()
        .justify_between()
        .gap(theme.layout.menu_item_padding_x)
        .px(theme.layout.menu_item_padding_x)
        .h(theme.menu_row_height)
        .items_center()
        .rounded(theme.radius.md)
        .text_color(color)
        .hover(|row| row.bg(theme.ui.menu_hover))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(move |view, _, window, cx| {
            view.menu = None;
            window.dispatch_action(Box::new(Dispatch(action.into())), cx);
            cx.notify();
        }))
        .child(div().min_w_0().truncate().child(label))
        .child(
            div()
                .flex_none()
                .map(|element| theme.ui_small_text(element))
                .text_color(theme.ui.panel_muted)
                .children(keys),
        )
}
