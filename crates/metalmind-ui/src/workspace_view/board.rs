//! `board::new`: una pizarra nueva y vacía. Con un nombre se crea en la raíz
//! del vault; sin él, la pregunta el panel lateral, como una nota.

use gpui_kit::{Context, Window};
use metalmind_board::Board;
use metalmind_keymap::PanelAction;
use metalmind_syntax::note_name;
use metalmind_vault::VaultPath;

use super::WorkspaceView;
use crate::actions::Dispatch;
use crate::panel::PanelView;
use crate::workspace::{Message, Placement, shown_path};

impl WorkspaceView {
    pub(super) fn new_board(
        &mut self,
        name: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match name.map(str::trim).filter(|name| !name.is_empty()) {
            Some(name) => self.create_board(name, window, cx),
            None => self.ask_board_name(window, cx),
        }
    }

    fn ask_board_name(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sidebar.read(cx).shown() == Some(PanelView::Folders) {
            self.focus_sidebar(window, cx);
        } else {
            self.toggle_panel(PanelView::Folders, window, cx);
        }
        window.dispatch_action(Box::new(Dispatch(PanelAction::NewBoard.into())), cx);
    }

    fn create_board(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let vault = self.session.read(cx).vault();
        let file = if note_name::is_board(name) {
            name.to_owned()
        } else {
            note_name::board_with_extension(name)
        };
        let created = VaultPath::new(&file)
            .map_err(|error| error.to_string())
            .and_then(|path| {
                vault
                    .create_board(&path, &Board::default().write())
                    .map_err(|error| error.to_string())
            });
        if let Err(error) = created {
            self.tell(Message::Error(error), cx);
            return self.focus(window, cx);
        }
        let path = shown_path(&vault.root().join(&file));
        let opened = self.workspace.update(cx, |workspace, cx| {
            cx.notify();
            workspace.open_file(&path, Placement::Current)
        });
        if let Err(error) = opened {
            self.workspace
                .update(cx, |workspace, _| workspace.report(&error));
        }
        self.reveal(&path, cx);
        self.sync_editors(cx);
        self.focus(window, cx);
        self.touch_focused(cx);
    }
}
