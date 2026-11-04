//! La raíz de la ventana: la barra de botones, el panel lateral, la lista de
//! notas, el árbol de paneles, un [`Editor`] por hoja, y el panel de contexto
//! a la derecha. Ejecuta los comandos de `:`,
//! `ctrl-w` y `space` sobre el [`Workspace`]. La lógica vive en el
//! workspace y en el panel; aquí solo se crean y sueltan editores y se
//! mueve el foco de GPUI adonde digan. Encima de los paneles flota, si está
//! abierto, el finder, la búsqueda de texto o la paleta, que tapa los toasts
//! de los avisos, arriba a la derecha.

mod autosave;
mod board;
mod buffer_edits;
mod commands;
mod context;
mod finder;
mod focus;
mod link_choice;
mod list_views;
mod messages;
mod new_note;
mod note_list;
mod overlay;
mod palette;
mod rename;
mod render;
mod reroute;
mod settings;
mod sidebar;
mod tag_change;
mod text_search;
mod toasts;
mod unresolved;
mod vault_undo;

use std::collections::HashMap;
use std::path::Path;

use gpui_kit::{AppContext as _, Context, Entity, Subscription, Task, Window};
use metalmind_config::Paths;

use crate::context_panel::ContextPanel;
use crate::editor::Editor;
use crate::finder::VaultSession;
use crate::math::Formulas;
use crate::new_note::NoteSettings;
use crate::note_list::NoteListView;
use crate::session::{self, VaultState};
use crate::sidebar::Sidebar;
use crate::workspace::{PaneId, Workspace};

use self::autosave::AutosaveTimer;
use self::finder::FinderVault;
use self::overlay::Overlay;
use self::tag_change::ParkedTagChange;
use self::toasts::ToastTimers;
use self::vault_undo::VaultUndo;

pub struct WorkspaceView {
    workspace: Entity<Workspace>,
    sidebar: Entity<Sidebar>,
    list: Entity<NoteListView>,
    context: Entity<ContextPanel>,
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
    toast_timers: ToastTimers,
    vault_undo: VaultUndo,
    /// La vista previa de tags que se dejó para abrir una línea.
    parked_tag_change: Option<ParkedTagChange>,
    /// El plan de un renombre que se está armando, antes de su vista previa.
    renaming: Option<Task<()>>,
    notes: NoteSettings,
    _subscriptions: [Subscription; 5],
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
        let sidebar = cx.new(|cx| Sidebar::new(&session, &state.panel, workspace.clone(), cx));
        let list = cx.new(|cx| NoteListView::new(&session, &state.list, workspace.clone(), cx));
        let context = cx.new(|cx| ContextPanel::new(&session, workspace.clone(), window, cx));
        let repaint = cx.observe(&workspace, |view, _, cx| {
            view.schedule_autosave(cx);
            view.refresh_change_preview(cx);
            cx.notify();
        });
        let activation = cx.observe_window_activation(window, Self::on_activation);
        let events = cx.subscribe_in(&sidebar, window, |view, _, event, window, cx| {
            view.on_sidebar_event(event.clone(), window, cx);
        });
        let list_events = cx.subscribe_in(&list, window, |view, _, event, window, cx| {
            view.on_list_event(event.clone(), window, cx);
        });
        let context_events = cx.subscribe_in(&context, window, |view, _, event, window, cx| {
            view.on_context_event(event, window, cx);
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
            context,
            editors: HashMap::from([(pane, first)]),
            title: String::new(),
            paths,
            vault,
            session,
            formulas,
            overlay: None,
            autosave: None,
            toast_timers: ToastTimers::default(),
            vault_undo: VaultUndo::default(),
            parked_tag_change: None,
            renaming: None,
            notes,
            _subscriptions: [repaint, events, list_events, context_events, activation],
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
}

#[cfg(test)]
#[path = "board_edit_tests.rs"]
mod board_edit_tests;
#[cfg(test)]
#[path = "board_tests.rs"]
mod board_tests;
