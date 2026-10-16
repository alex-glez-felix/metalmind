//! El editor de gpui-component, para comparar con la vista propia.

use gpui_kit::component::RopeExt as _;
use gpui_kit::component::input::{Editor, EditorState, Position};
use gpui_kit::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div,
};

use crate::hud::Status;

/// Solo guarda el handle del estado; clonarla es barato.
#[derive(Clone)]
pub struct KitEditor {
    state: Entity<EditorState>,
}

impl KitEditor {
    pub fn new(text: &str, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|cx| EditorState::new(window, cx).default_value(text.to_owned()));
        Self { state }
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.state.update(cx, |state, cx| state.focus(window, cx));
    }

    pub fn status(&self, cx: &App) -> Status {
        let state = self.state.read(cx);
        let position = state.cursor_position();
        Status {
            mode: "gpui-component",
            line: to_usize(position.line) + 1,
            column: to_usize(position.character) + 1,
            lines: state.text().lines_len(),
            ..Status::default()
        }
    }

    /// Para el benchmark: baja el cursor y deja que el editor lo siga.
    pub fn move_down(&self, lines: u32, window: &mut Window, cx: &mut App) {
        self.state.update(cx, |state, cx| {
            let line = state.cursor_position().line.saturating_add(lines);
            state.set_cursor_position(Position::new(line, 0), window, cx);
        });
    }
}

fn to_usize(value: u32) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

impl Render for KitEditor {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(Editor::new(&self.state).h_full())
    }
}
