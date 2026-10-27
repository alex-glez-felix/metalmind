//! La raíz de la ventana: la barra de botones, el panel lateral, la lista de
//! notas y el árbol de paneles, un [`Editor`] por hoja. Ejecuta los comandos de `:`,
//! `ctrl-w` y `space` sobre el [`Workspace`]. La lógica vive en el
//! workspace y en el panel; aquí solo se crean y sueltan editores y se
//! mueve el foco de GPUI adonde digan. Encima de los paneles flota, si está
//! abierto, el finder, la búsqueda de texto o la paleta.

mod autosave;
mod finder;
mod focus;
mod link_choice;
mod new_note;
mod note_list;
mod overlay;
mod palette;
mod reroute;
mod tag_change;
mod text_search;
mod unresolved;

use std::collections::HashMap;
use std::path::Path;

use gpui_kit::{
    AnyElement, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, Styled as _, Subscription, Window, div,
    prelude::FluentBuilder as _,
};
use metalmind_config::Paths;
use metalmind_keymap::{Action, Invocation};

use crate::actions::Dispatch;
use crate::clipboard::SystemClipboard;
use crate::editor::Editor;
use crate::finder::VaultSession;
use crate::frames::FrameClock;
use crate::math::Formulas;
use crate::new_note::NoteSettings;
use crate::note_list::NoteListView;
use crate::panel::PanelView;
use crate::session::{self, VaultState};
use crate::sidebar::{self, Sidebar, SidebarEvent};
use crate::theme::Theme;
use crate::workspace::{Axis, Direction, Effect, Message, Node, PaneId, Workspace};

use self::autosave::AutosaveTimer;
use self::finder::FinderVault;
use self::overlay::Overlay;
use self::reroute::{Reroute, reroute};
use self::tag_change::LastTagChange;

pub struct WorkspaceView {
    workspace: Entity<Workspace>,
    sidebar: Entity<Sidebar>,
    list: Entity<NoteListView>,
    editors: HashMap<PaneId, Entity<Editor>>,
    title: String,
    /// Dónde guardar el estado de cada vault; `None` si no hay `HOME`.
    paths: Option<Paths>,
    vault: FinderVault,
    session: Entity<VaultSession>,
    /// Las fórmulas, compartidas por los paneles de la ventana.
    formulas: Entity<Formulas>,
    overlay: Option<Overlay>,
    autosave: Option<AutosaveTimer>,
    last_tag_change: Option<LastTagChange>,
    notes: NoteSettings,
    _subscriptions: [Subscription; 4],
}

impl WorkspaceView {
    /// El panel lateral y la lista como los dejó `state`.
    pub fn new(
        workspace: Entity<Workspace>,
        session: Entity<VaultSession>,
        state: &VaultState,
        paths: Option<Paths>,
        notes: NoteSettings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let sidebar =
            cx.new(|cx| Sidebar::new(&session, &state.panel, workspace.clone(), window, cx));
        let list = cx.new(|cx| NoteListView::new(&session, &state.list, workspace.clone(), cx));
        let repaint = cx.observe(&workspace, |view, _, cx| {
            view.schedule_autosave(cx);
            view.refresh_tag_change(cx);
            cx.notify();
        });
        let activation = cx.observe_window_activation(window, Self::on_activation);
        let events = cx.subscribe_in(&sidebar, window, |view, _, event, window, cx| {
            view.on_sidebar_event(event.clone(), window, cx);
        });
        let list_events = cx.subscribe_in(&list, window, |view, _, event, window, cx| {
            view.on_list_event(event.clone(), window, cx);
        });
        let pane = workspace.read(cx).focused();
        let formulas = cx.new(|_| Formulas::default());
        let first = cx.new(|cx| {
            Editor::new(
                workspace.clone(),
                session.clone(),
                formulas.clone(),
                pane,
                cx,
            )
        });
        let vault = FinderVault::new(session.clone(), &workspace, cx);
        Self {
            workspace,
            sidebar,
            list,
            editors: HashMap::from([(pane, first)]),
            title: String::new(),
            paths,
            vault,
            session,
            formulas,
            overlay: None,
            autosave: None,
            last_tag_change: None,
            notes,
            _subscriptions: [repaint, events, list_events, activation],
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

    /// Selecciona `file` en el árbol del panel lateral.
    pub fn reveal(&self, file: &std::path::Path, cx: &mut Context<Self>) {
        self.sidebar
            .update(cx, |sidebar, cx| sidebar.reveal(file, cx));
    }

    #[must_use]
    pub fn sidebar_visible(&self, cx: &Context<Self>) -> bool {
        self.sidebar.read(cx).is_visible()
    }

    pub fn focus_sidebar(&self, window: &mut Window, cx: &mut Context<Self>) {
        let handle = self.sidebar.read(cx).focus_handle().clone();
        window.focus(&handle, cx);
    }

    /// Lo consulta la ventana cuando el compositor pide cerrarla. Si hay
    /// cambios, el workspace muestra la nota que los tiene.
    pub fn may_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let closes = self.workspace.update(cx, |workspace, cx| {
            cx.notify();
            workspace.may_close_window()
        });
        if closes {
            self.save_state(cx);
        } else {
            self.sync_editors(cx);
            self.focus(window, cx);
        }
        closes
    }

    fn save_state(&self, cx: &Context<Self>) {
        let Some(paths) = &self.paths else {
            return;
        };
        let sidebar = self.sidebar.read(cx);
        let root = sidebar.root();
        let state = VaultState {
            panel: sidebar.state(),
            list: self.list.read(cx).state(),
        };
        if let Err(error) = state.save(paths, root) {
            log::warn!("no se guardó el estado del vault: {error}");
        }
        self.save_jumps(paths, root, cx);
    }

    /// La jumplist y las marks `A`–`Z`, para la próxima vez que se abra el
    /// vault de `root`.
    fn save_jumps(&self, paths: &Paths, root: &Path, cx: &Context<Self>) {
        match self.workspace.read(cx).saved_jumps(root) {
            Ok(jumps) => {
                if let Err(error) = jumps.save(root, &session::jumps_file(paths, root)) {
                    log::warn!("no se guardaron los saltos: {error}");
                }
            }
            Err(error) => log::warn!("no se guardaron los saltos: {error}"),
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
            Effect::OpenTextSearch => {
                self.sync_editors(cx);
                self.open_text_search(window, cx);
            }
            Effect::OpenCommandPalette => {
                self.sync_editors(cx);
                self.open_palette(window, cx);
            }
            Effect::OpenUnresolvedLinks => {
                self.sync_editors(cx);
                self.open_unresolved_links(window, cx);
            }
            Effect::TogglePanel(view) => self.toggle_panel(view, window, cx),
            Effect::ToggleNoteList => self.toggle_list(window, cx),
            Effect::ChooseListSource => {
                self.sync_editors(cx);
                self.choose_list_source(window, cx);
            }
            Effect::FocusPastEdge(Direction::Left) if self.list.read(cx).is_visible() => {
                self.focus_list(window, cx);
            }
            Effect::FocusPastEdge(Direction::Left) if self.sidebar.read(cx).is_visible() => {
                self.focus_sidebar(window, cx);
            }
            Effect::FocusPastEdge(_) => {}
            Effect::ChooseLink => {
                self.sync_editors(cx);
                self.open_link_choice(window, cx);
            }
            Effect::Dispatch(action) => {
                self.sync_editors(cx);
                window.dispatch_action(Box::new(Dispatch(action.into())), cx);
            }
            Effect::ChangeTags(request) => {
                self.sync_editors(cx);
                self.change_tags(&request, window, cx);
            }
            Effect::UndoTagChange => {
                self.sync_editors(cx);
                self.undo_tag_change(cx);
                self.focus(window, cx);
            }
            Effect::NewNote(request) => {
                self.sync_editors(cx);
                self.new_note(request, window, cx);
            }
            Effect::OpenDailyNote => {
                self.sync_editors(cx);
                self.open_daily_note(window, cx);
            }
            Effect::OpenTagPage(tag) => {
                self.sync_editors(cx);
                self.open_tag_page(&tag, window, cx);
            }
            Effect::SetFocus(focus) => {
                self.sync_editors(cx);
                self.set_focus(focus, window, cx);
            }
            Effect::ShowFocus => {
                self.sync_editors(cx);
                self.show_focus(window, cx);
            }
        }
    }

    /// Una acción que llegó a la raíz sin que la atendiera su vista.
    fn reroute(&mut self, invocation: &Invocation, window: &mut Window, cx: &mut Context<Self>) {
        let sidebar = self.sidebar.read(cx).is_visible();
        let list = self.list.read(cx).is_visible();
        match reroute(
            invocation.action.owner(),
            sidebar,
            list,
            self.overlay.is_some(),
        ) {
            Reroute::Sidebar => self.focus_sidebar(window, cx),
            Reroute::List => self.focus_list(window, cx),
            Reroute::Editor => self.focus(window, cx),
            Reroute::Nowhere => {
                let name = invocation.action.name();
                let error = format!("{name} no tiene a quién llegar: su vista no está abierta");
                self.workspace.update(cx, |workspace, _| {
                    workspace.set_message(Message::Error(error));
                });
                cx.notify();
                return;
            }
        }
        window.dispatch_action(Box::new(Dispatch(invocation.clone())), cx);
    }

    fn toggle_panel(&mut self, view: PanelView, window: &mut Window, cx: &mut Context<Self>) {
        let visible = self
            .sidebar
            .update(cx, |sidebar, cx| sidebar.toggle(view, window, cx));
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
            SidebarEvent::Opened { focus_editor } => {
                self.sync_editors(cx);
                if focus_editor {
                    self.focus(window, cx);
                }
                self.touch_focused(cx);
            }
            SidebarEvent::PanesChanged => self.sync_editors(cx),
            SidebarEvent::FocusEditor => self.focus(window, cx),
            SidebarEvent::ChangeTags(request) => self.change_tags(&request, window, cx),
            SidebarEvent::NewTaggedNote(tag) => self.new_tagged_note(tag, window, cx),
            SidebarEvent::OpenTagPage(tag) => self.open_tag_page(&tag, window, cx),
            SidebarEvent::CreateNote(path) => self.create_panel_note(&path, window, cx),
        }
        cx.notify();
    }

    /// Un editor por panel del árbol, ni uno más.
    fn sync_editors(&mut self, cx: &mut Context<Self>) {
        let panes = self.workspace.read(cx).tree().panes();
        self.editors.retain(|pane, _| panes.contains(pane));
        for pane in panes {
            let (workspace, session, formulas) = (&self.workspace, &self.session, &self.formulas);
            self.editors.entry(pane).or_insert_with(|| {
                cx.new(|cx| {
                    Editor::new(
                        workspace.clone(),
                        session.clone(),
                        formulas.clone(),
                        pane,
                        cx,
                    )
                })
            });
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
        let theme = Theme::get(cx).clone();
        let width = window.viewport_size().width;
        self.list.update(cx, |list, _| {
            list.settle_visibility(width, theme.layout.list_open_min_width);
        });
        let root = self.workspace.read(cx).tree().root().clone();
        let element = div()
            .size_full()
            .font_family(theme.font_family.clone())
            .map(|element| theme.ui_text(element))
            .text_color(theme.ui.foreground)
            .bg(theme.ui.background)
            // Los comandos y las acciones del motor llegan aquí desde
            // cualquier vista; lo demás llega solo si su vista no tenía el
            // foco.
            .on_action(
                cx.listener(|view, Dispatch(invocation): &Dispatch, window, cx| {
                    match invocation.action {
                        Action::Command(_) => view.run(invocation, window, cx),
                        Action::Vim(action) => view.execute_vim(action, window, cx),
                        _ => view.reroute(invocation, window, cx),
                    }
                }),
            );
        let shown = self.sidebar.read(cx).shown();
        let list = self.list.read(cx).is_visible();
        element
            .relative()
            .flex()
            .flex_row()
            .child(sidebar::rail(shown, list, &theme))
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
            .children(self.overlay.as_ref().map(|overlay| overlay.render(&theme)))
    }
}
