//! Cómo se pinta la ventana: el riel, el panel lateral, la lista, el árbol
//! de paneles con un editor por hoja, el panel de contexto, los toasts y,
//! encima, lo que esté abierto.

use gpui_kit::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    Styled as _, Window, div, prelude::FluentBuilder as _,
};

use super::WorkspaceView;
use crate::actions::Dispatch;
use crate::frames::FrameClock;
use crate::sidebar;
use crate::theme::Theme;
use crate::workspace::{Axis, Node};

impl WorkspaceView {
    fn update_title(&mut self, window: &mut Window, cx: &Context<Self>) {
        let workspace = self.workspace.read(cx);
        let Ok(view) = workspace.view(workspace.focused()) else {
            return;
        };
        let modified = if view.unsaved() { " [+]" } else { "" };
        let title = format!("{}{modified} — Metalmind", view.document.display_name());
        if title != self.title {
            window.set_window_title(&title);
            self.title = title;
        }
    }

    fn render_node(&self, node: &Node, theme: &Theme) -> AnyElement {
        match node {
            Node::Pane(pane) => div()
                .size_full()
                .children(self.editors.get(pane).cloned())
                .into_any_element(),
            Node::Split { axis, children } => {
                let cells = children.iter().enumerate().map(|(index, child)| {
                    div()
                        .flex_1()
                        .min_w_0()
                        .min_h_0()
                        .border_color(theme.ui.pane_border)
                        .when(index > 0, |cell| match axis {
                            Axis::Row => cell.border_l(theme.pane_border_width),
                            Axis::Column => cell.border_t(theme.pane_border_width),
                        })
                        .child(self.render_node(child, theme))
                });
                div()
                    .size_full()
                    .flex()
                    .map(|split| match axis {
                        Axis::Row => split.flex_row(),
                        Axis::Column => split.flex_col(),
                    })
                    .children(cells)
                    .into_any_element()
            }
        }
    }
}

impl Render for WorkspaceView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        FrameClock::render_started(cx);
        // Un frame de la ventana, no de cada panel: lo que pide cualquier
        // panel en este frame no sale de la caché.
        self.formulas
            .update(cx, |formulas, _| formulas.next_frame());
        self.update_title(window, cx);
        // Aquí y no al cambiar el workspace: un aviso que llega sin avisar
        // también tiene su temporizador, que cuenta desde que se ve.
        self.schedule_toasts(cx);
        let theme = Theme::get(cx).clone();
        let width = window.viewport_size().width;
        self.list.update(cx, |list, cx| {
            list.settle_visibility(width, theme.layout.list_open_min_width, cx);
        });
        let root = self.workspace.read(cx).tree().root().clone();
        let element = div()
            .size_full()
            .font_family(theme.font_family.clone())
            .map(|element| theme.ui_text(element))
            .text_color(theme.ui.foreground)
            .bg(theme.ui.background)
            .on_action(
                cx.listener(|view, Dispatch(invocation): &Dispatch, window, cx| {
                    view.dispatch(invocation, window, cx);
                }),
            );
        let shown = self.sidebar.read(cx).shown();
        let list = self.list.read(cx).is_visible();
        let context = self.context.read(cx).is_visible();
        element
            .relative()
            .flex()
            .flex_row()
            .child(sidebar::rail(shown, context, list, &theme))
            .when(shown.is_some(), |element| {
                element.child(self.sidebar.clone())
            })
            .when(list, |element| element.child(self.list.clone()))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(self.render_node(&root, &theme)),
            )
            .when(context, |element| element.child(self.context.clone()))
            .children(self.render_toasts(&theme, cx))
            .children(self.overlay.as_ref().map(|overlay| overlay.render(&theme)))
    }
}
