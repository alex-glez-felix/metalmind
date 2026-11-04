//! El cuerpo de un panel, texto o pizarra; de la pizarra, quién la crea, qué se
//! ve de ella en la barra de estado y por dónde le llegan sus acciones.

use gpui_kit::{
    App, AppContext as _, Context, CursorStyle, Div, ElementInputHandler, Entity,
    InteractiveElement as _, KeyContext, Keystroke, MouseButton, ParentElement as _, SharedString,
    Styled as _, Window, canvas, div, prelude::FluentBuilder as _,
};
use metalmind_keymap::{BoardAction, context};
use metalmind_vim::Mode;

use super::Editor;
use super::element::EditorElement;
use crate::board::BoardView;
use crate::frames::FrameClock;
use crate::theme::Theme;
use crate::workspace::Message;

impl Editor {
    /// El cuerpo del panel: la pizarra, si la muestra, o el texto.
    pub(super) fn body(&self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let body = div()
            .relative()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .map(|element| theme.editor_text(element));
        let Some(board) = self.board.clone() else {
            return body
                .cursor(CursorStyle::IBeam)
                .on_scroll_wheel(cx.listener(Self::on_scroll))
                .on_mouse_down(MouseButton::Left, cx.listener(Self::on_left_down))
                .on_mouse_move(cx.listener(Self::on_mouse_move))
                .on_mouse_up(MouseButton::Left, cx.listener(Self::on_left_up))
                .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_left_up))
                .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_middle_down))
                .child(EditorElement::new(cx.entity(), theme.clone()));
        };
        self.board_body(body, board, cx)
    }

    /// El texto de una caja está abierto en el motor, desde este panel.
    pub(super) fn board_text_open(&self, cx: &App) -> bool {
        self.board.is_some()
            && self
                .view(cx)
                .is_some_and(|view| view.focused && view.document.open_board_text().is_some())
    }

    /// Los contextos de teclas de la pizarra, según lo que esté abierto.
    pub(super) fn board_key_context(&self, key_context: &mut KeyContext, cx: &App) {
        let Some(board) = &self.board else {
            return;
        };
        let normal = self.mode(cx) == Mode::Normal;
        if self.board_text_open(cx) {
            key_context.add(context::BOARD_TEXT);
            if normal && self.workspace.read(cx).engine_is_idle() {
                key_context.add(context::BOARD_TEXT_IDLE);
            }
        } else if normal {
            // Con la línea de `:` abierta las letras y `enter` son de ella.
            key_context.add(if board.read(cx).hints_open() {
                context::BOARD_HINTS
            } else {
                context::BOARD
            });
        }
    }

    /// Una letra con las pistas de unir a la vista. `true` si era para ellas.
    pub(super) fn feed_board_hint(&self, keystroke: &Keystroke, cx: &mut Context<Self>) -> bool {
        let Some(board) = self
            .board
            .clone()
            .filter(|board| board.read(cx).hints_open())
        else {
            return false;
        };
        let key = keystroke
            .key_char
            .as_deref()
            .and_then(|text| text.chars().next());
        if let Some(key) = key {
            board.update(cx, |board, cx| board.hint_key(key, cx));
        }
        true
    }

    /// Una pizarra por panel que la muestra, ni una más.
    pub(super) fn sync_board(&mut self, cx: &mut Context<Self>) {
        let shows_board = self.view(cx).is_some_and(|view| view.document.is_board());
        if !shows_board {
            self.board = None;
        } else if self.board.is_none() {
            let (workspace, session, pane) =
                (self.workspace.clone(), self.session.clone(), self.pane);
            self.board = Some(cx.new(|_| BoardView::new(workspace, session, pane)));
        }
    }

    pub(super) fn position(&self, line: usize, column: usize, cx: &App) -> SharedString {
        match self.board.as_ref() {
            Some(board) => board
                .read(cx)
                .zoom_percent()
                .map_or_else(SharedString::default, |zoom| format!("{zoom} %").into()),
            None => format!("{line}:{column}").into(),
        }
    }

    pub(super) fn on_board_action(
        &mut self,
        action: BoardAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        FrameClock::input(cx);
        match &self.board {
            Some(board) => board.update(cx, |board, cx| board.run(action, window, cx)),
            // Llega desde la paleta o por su nombre, con una nota en el panel.
            None => self.update_workspace(cx, |workspace, _, _| {
                workspace.show_message(Message::Info("Este panel no muestra una pizarra".into()));
            }),
        }
    }

    fn board_body(&self, body: Div, board: Entity<BoardView>, cx: &mut Context<Self>) -> Div {
        let preedit = self.marked.clone();
        board.update(cx, |board, _| board.set_preedit(preedit));
        // Lo que se teclea tras `:` llega por el input handler, y no hay
        // `EditorElement` que lo registre.
        let (focus, editor) = (self.focus_handle.clone(), cx.entity());
        let input = canvas(
            |_, _, _| {},
            move |bounds, (), window, cx| {
                window.handle_input(&focus, ElementInputHandler::new(bounds, editor), cx);
            },
        )
        .absolute()
        .size_full();
        // En la captura: la barra de zoom no deja que su clic llegue a la pizarra,
        // y el panel tiene que tomar el foco antes de que el botón despache.
        body.capture_any_mouse_down(
            cx.listener(|editor, _, window, cx| editor.take_focus(window, cx)),
        )
        .child(input)
        .child(board)
    }
}
