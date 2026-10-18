//! La raíz de la ventana: la barra de botones, el panel lateral y el árbol
//! de paneles, un [`Editor`] por hoja. Ejecuta los comandos de `:`,
//! `ctrl-w` y `space` sobre el [`Workspace`]. La lógica vive en el
//! workspace y en el panel; aquí solo se crean y sueltan editores y se
//! mueve el foco de GPUI adonde digan. Encima de los paneles flota, si está
//! abierto, el finder o la paleta.

mod finder;
mod link_choice;
mod overlay;
mod palette;

use std::collections::HashMap;
use std::time::Instant;

use gpui_kit::{
    AnyElement, AppContext as _, Context, Div, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, Styled as _, Subscription, Window, div,
    prelude::FluentBuilder as _, relative,
};
use metalmind_config::Paths;
use metalmind_vim::Invocation;

use crate::actions::{
    CommandAction, Edit, FocusDown, FocusLeft, FocusNext, FocusRight, FocusUp, FollowLink,
    ForceEdit, ForceQuit, Only, OpenCommandPalette, OpenFinder, Quit, Save, SaveAndQuit, SaveAs,
    SaveAsAndQuit, Set, SplitDown, SplitRight, ToggleFolders, ToggleTags,
};
use crate::clipboard::SystemClipboard;
use crate::editor::Editor;
use crate::finder::VaultSession;
use crate::panel::PanelView;
use crate::session::VaultState;
use crate::sidebar::{self, Sidebar, SidebarEvent};
use crate::theme::Theme;
use crate::workspace::{Axis, Direction, Effect, Node, PaneId, Workspace};

use self::finder::FinderVault;
use self::overlay::Overlay;

pub struct WorkspaceView {
    workspace: Entity<Workspace>,
    sidebar: Entity<Sidebar>,
    editors: HashMap<PaneId, Entity<Editor>>,
    title: String,
    /// Dónde guardar el estado de cada vault; `None` si no hay `HOME`.
    paths: Option<Paths>,
    vault: FinderVault,
    overlay: Option<Overlay>,
    _subscriptions: [Subscription; 2],
}

impl WorkspaceView {
    pub fn new(
        workspace: Entity<Workspace>,
        sidebar: Entity<Sidebar>,
        session: Entity<VaultSession>,
        paths: Option<Paths>,
        launched: Instant,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let repaint = cx.observe(&workspace, |_, _, cx| cx.notify());
        let events = cx.subscribe_in(&sidebar, window, |view, _, event, window, cx| {
            view.on_sidebar_event(*event, window, cx);
        });
        let pane = workspace.read(cx).focused();
        let first = cx.new(|cx| Editor::new(workspace.clone(), pane, Some(launched), cx));
        let vault = FinderVault::new(session, &workspace, cx);
        Self {
            workspace,
            sidebar,
            editors: HashMap::from([(pane, first)]),
            title: String::new(),
            paths,
            vault,
            overlay: None,
            _subscriptions: [repaint, events],
        }
    }

    /// Le da el foco de GPUI al editor del panel enfocado, o al finder o
    /// la paleta si hay uno abierto.
    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(overlay) = &self.overlay {
            window.focus(&overlay.focus_handle(cx), cx);
            return;
        }
        let focused = self.workspace.read(cx).focused();
        if let Some(editor) = self.editors.get(&focused) {
            let handle = editor.read(cx).focus_handle().clone();
            window.focus(&handle, cx);
        }
    }

    pub fn focus_sidebar(&self, window: &mut Window, cx: &mut Context<Self>) {
        let handle = self.sidebar.read(cx).focus_handle().clone();
        window.focus(&handle, cx);
    }

    /// Lo consulta la ventana cuando el compositor pide cerrarla.
    pub fn may_close(&mut self, cx: &mut Context<Self>) -> bool {
        let closes = self.workspace.update(cx, |workspace, cx| {
            cx.notify();
            workspace.may_close_window()
        });
        if closes {
            self.save_state(cx);
        }
        closes
    }

    fn save_state(&self, cx: &Context<Self>) {
        let Some(paths) = &self.paths else {
            return;
        };
        let sidebar = self.sidebar.read(cx);
        let state = VaultState {
            panel: sidebar.state(),
        };
        if let Err(error) = state.save(paths, sidebar.root()) {
            log::warn!("no se guardó el estado del vault: {error}");
        }
    }

    fn run(&mut self, invocation: &Invocation, window: &mut Window, cx: &mut Context<Self>) {
        let effect = self.workspace.update(cx, |workspace, cx| {
            cx.notify();
            workspace.run(invocation, &mut SystemClipboard(cx))
        });
        match effect {
            Effect::CloseWindow => {
                self.save_state(cx);
                window.remove_window();
            }
            Effect::Stay => {
                self.sync_editors(cx);
                self.focus(window, cx);
                self.touch_focused(cx);
            }
            // Una macro pudo cambiar los paneles antes de abrir el picker.
            Effect::OpenFinder => {
                self.sync_editors(cx);
                self.open_finder(window, cx);
            }
            Effect::OpenCommandPalette => {
                self.sync_editors(cx);
                self.open_palette(window, cx);
            }
            Effect::TogglePanel(view) => self.toggle_panel(view, window, cx),
            Effect::FocusPastEdge(Direction::Left) if self.sidebar.read(cx).is_visible() => {
                self.focus_sidebar(window, cx);
            }
            Effect::FocusPastEdge(_) => {}
            Effect::ChooseLink => {
                self.sync_editors(cx);
                self.open_link_choice(window, cx);
            }
        }
    }

    fn toggle_panel(&mut self, view: PanelView, window: &mut Window, cx: &mut Context<Self>) {
        let visible = self
            .sidebar
            .update(cx, |sidebar, cx| sidebar.toggle(view, cx));
        if visible {
            self.focus_sidebar(window, cx);
        } else {
            self.focus(window, cx);
        }
        cx.notify();
    }

    fn on_sidebar_event(
        &mut self,
        event: SidebarEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            SidebarEvent::Opened => {
                self.sync_editors(cx);
                self.focus(window, cx);
                self.touch_focused(cx);
            }
            SidebarEvent::PanesChanged => self.sync_editors(cx),
            SidebarEvent::FocusEditor => self.focus(window, cx),
        }
        cx.notify();
    }

    /// Un editor por panel del árbol, ni uno más.
    fn sync_editors(&mut self, cx: &mut Context<Self>) {
        let panes = self.workspace.read(cx).tree().panes();
        self.editors.retain(|pane, _| panes.contains(pane));
        for pane in panes {
            let workspace = &self.workspace;
            self.editors
                .entry(pane)
                .or_insert_with(|| cx.new(|cx| Editor::new(workspace.clone(), pane, None, cx)));
        }
        cx.notify();
    }

    fn show(&mut self, overlay: Overlay, window: &mut Window, cx: &mut Context<Self>) {
        self.overlay = Some(overlay);
        self.focus(window, cx);
        cx.notify();
    }

    fn close_overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.overlay = None;
        self.focus(window, cx);
        cx.notify();
    }

    fn update_title(&mut self, window: &mut Window, cx: &Context<Self>) {
        let Ok(document) = self.workspace.read(cx).focused_document() else {
            return;
        };
        let modified = if document.is_modified() { " [+]" } else { "" };
        let title = format!("{}{modified} — Metalmind", document.display_name());
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
        element = on_command::<SaveAs>(element, cx);
        element = on_command::<SaveAsAndQuit>(element, cx);
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
        element = on_command::<Set>(element, cx);
        element = on_command::<OpenFinder>(element, cx);
        element = on_command::<OpenCommandPalette>(element, cx);
        element = on_command::<ToggleFolders>(element, cx);
        element = on_command::<ToggleTags>(element, cx);
        element = on_command::<FollowLink>(element, cx);
        let sidebar = self.sidebar.read(cx);
        let shown = sidebar.shown();
        element
            .relative()
            .flex()
            .flex_row()
            .child(sidebar::rail(shown, &theme))
            .when(shown.is_some(), |element| {
                element.child(self.sidebar.clone())
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(self.render_node(&root, &theme)),
            )
            .children(self.overlay.as_ref().map(|overlay| overlay.render(&theme)))
    }
}
