//! La raíz de la ventana: pinta el árbol de paneles, un [`Editor`] por
//! hoja, y ejecuta los comandos de `:` y `ctrl-w` sobre el [`Workspace`].
//! La lógica vive en el workspace; aquí solo se crean y sueltan editores y
//! se mueve el foco de GPUI adonde diga.

use std::collections::HashMap;
use std::time::Instant;

use gpui_kit::{
    AnyElement, AppContext as _, Context, Div, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, Styled as _, Subscription, Window, div,
    prelude::FluentBuilder as _, relative,
};
use nota_vim::Invocation;

use crate::actions::{
    CommandAction, Edit, FocusDown, FocusLeft, FocusNext, FocusRight, FocusUp, ForceEdit,
    ForceQuit, Only, Quit, Save, SaveAndQuit, SplitDown, SplitRight,
};
use crate::clipboard::SystemClipboard;
use crate::document::Document;
use crate::editor::Editor;
use crate::theme::Theme;
use crate::workspace::{Axis, Effect, Node, PaneId, Workspace};

pub struct WorkspaceView {
    workspace: Entity<Workspace>,
    editors: HashMap<PaneId, Entity<Editor>>,
    title: String,
    _repaint: Subscription,
}

impl WorkspaceView {
    pub fn new(document: Document, launched: Instant, cx: &mut Context<Self>) -> Self {
        let workspace = cx.new(|_| Workspace::new(document));
        let repaint = cx.observe(&workspace, |_, _, cx| cx.notify());
        let pane = workspace.read(cx).focused();
        let first = cx.new(|cx| Editor::new(workspace.clone(), pane, Some(launched), cx));
        Self {
            workspace,
            editors: HashMap::from([(pane, first)]),
            title: String::new(),
            _repaint: repaint,
        }
    }

    /// Le da el foco de GPUI al editor del panel enfocado.
    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        let focused = self.workspace.read(cx).focused();
        if let Some(editor) = self.editors.get(&focused) {
            let handle = editor.read(cx).focus_handle().clone();
            window.focus(&handle, cx);
            editor.update(cx, Editor::reveal_cursor);
        }
    }

    /// Lo consulta la ventana cuando el compositor pide cerrarla.
    pub fn may_close(&mut self, cx: &mut Context<Self>) -> bool {
        self.workspace.update(cx, |workspace, cx| {
            cx.notify();
            workspace.may_close_window()
        })
    }

    fn run(&mut self, invocation: &Invocation, window: &mut Window, cx: &mut Context<Self>) {
        let effect = self.workspace.update(cx, |workspace, cx| {
            cx.notify();
            workspace.run(invocation, &mut SystemClipboard(cx))
        });
        match effect {
            Effect::CloseWindow => window.remove_window(),
            Effect::Stay => self.sync_editors(window, cx),
        }
    }

    /// Un editor por panel del árbol, ni uno más.
    fn sync_editors(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let panes = self.workspace.read(cx).tree().panes();
        self.editors.retain(|pane, _| panes.contains(pane));
        for pane in panes {
            let workspace = &self.workspace;
            self.editors
                .entry(pane)
                .or_insert_with(|| cx.new(|cx| Editor::new(workspace.clone(), pane, None, cx)));
        }
        self.focus(window, cx);
        cx.notify();
    }

    fn update_title(&mut self, window: &mut Window, cx: &Context<Self>) {
        let Ok(document) = self.workspace.read(cx).focused_document() else {
            return;
        };
        let modified = if document.is_modified() { " [+]" } else { "" };
        let title = format!("{}{modified} — nota", document.path().display());
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
                        .border_color(theme.pane_border)
                        .when(index > 0, |cell| match axis {
                            Axis::Row => cell.border_l(theme.border_width),
                            Axis::Column => cell.border_t(theme.border_width),
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

/// Cada comando con nombre llega aquí desde el editor enfocado.
fn on_command<A: CommandAction>(root: Div, cx: &mut Context<WorkspaceView>) -> Div {
    root.on_action(cx.listener(|view, action: &A, window, cx| {
        view.run(&action.invocation(), window, cx);
    }))
}

impl Render for WorkspaceView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.update_title(window, cx);
        let theme = Theme::get(cx).clone();
        let root = self.workspace.read(cx).tree().root().clone();
        let mut element = div()
            .size_full()
            .font_family(theme.font_family.clone())
            .text_size(theme.font_size)
            .line_height(relative(theme.line_height))
            .text_color(theme.foreground)
            .bg(theme.background);
        element = on_command::<Save>(element, cx);
        element = on_command::<Quit>(element, cx);
        element = on_command::<ForceQuit>(element, cx);
        element = on_command::<SaveAndQuit>(element, cx);
        element = on_command::<Edit>(element, cx);
        element = on_command::<ForceEdit>(element, cx);
        element = on_command::<SplitRight>(element, cx);
        element = on_command::<SplitDown>(element, cx);
        element = on_command::<FocusLeft>(element, cx);
        element = on_command::<FocusDown>(element, cx);
        element = on_command::<FocusUp>(element, cx);
        element = on_command::<FocusRight>(element, cx);
        element = on_command::<FocusNext>(element, cx);
        element = on_command::<Only>(element, cx);
        element.child(self.render_node(&root, &theme))
    }
}
