//! `board::new`: una pizarra nueva y vacía. Con un nombre se crea en la raíz
//! del vault; sin él, la pregunta el panel lateral, como una nota. Y elegir la
//! nota de una tarjeta nueva, con las notas del finder.

use gpui_kit::{App, AppContext as _, Context, Entity, Window};
use metalmind_board::Board;
use metalmind_keymap::PanelAction;
use metalmind_syntax::note_name;
use metalmind_vault::VaultPath;

use super::WorkspaceView;
use super::overlay::Overlay;
use crate::actions::Dispatch;
use crate::board::BoardView;
use crate::finder::Note;
use crate::panel::PanelView;
use crate::picker::{Fuzzy, Haystack, PickerEvent, PickerOptions, PickerState, PickerView};
use crate::theme::Theme;
use crate::workspace::{Message, Placement, shown_path};

/// El picker de la nota de una tarjeta nueva.
pub type CardPicker = PickerView<Fuzzy<Note>>;

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

    fn focused_board(&self, cx: &App) -> Option<Entity<BoardView>> {
        let pane = self.workspace.read(cx).focused();
        self.editors.get(&pane)?.read(cx).board().cloned()
    }

    pub(super) fn open_card_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.focused_board(cx).is_none() {
            return self.tell(
                Message::Info("Este panel no muestra una pizarra".to_owned()),
                cx,
            );
        }
        let notes = self.session.read(cx).notes();
        let fuzzy = Fuzzy::new(
            PickerState::new(notes),
            Haystack::Paths,
            "No hay notas".into(),
        );
        let options = PickerOptions {
            placeholder: "La nota de la tarjeta…".into(),
            splits: false,
            focus: None,
        };
        let picker = cx.new(|cx| PickerView::new(fuzzy, options, cx));
        let events = cx.subscribe_in(&picker, window, Self::on_card_picker_event);
        self.show(
            Overlay::NoteCard {
                picker,
                _events: events,
            },
            window,
            cx,
        );
    }

    fn on_card_picker_event(
        &mut self,
        picker: &Entity<CardPicker>,
        event: &PickerEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let chosen = match event {
            PickerEvent::Dismissed => None,
            PickerEvent::Chosen(chosen) => picker
                .read(cx)
                .delegate()
                .items()
                .get(chosen.item)
                .map(|note| note.0.as_str().to_owned()),
        };
        self.close_overlay(window, cx);
        self.focus(window, cx);
        let Some(file) = chosen else {
            return;
        };
        let Some(board) = self.focused_board(cx) else {
            return;
        };
        let theme = Theme::get(cx).clone();
        board.update(cx, |board, cx| board.insert_card(&file, &theme, cx));
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
