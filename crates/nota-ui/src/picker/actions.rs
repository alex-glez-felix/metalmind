//! Las acciones de un picker y sus atajos: moverse por la lista como en
//! Telescope y Zed, y editar el input como en readline.

use gpui_kit::{Action, App, KeyBinding};

/// Contexto de teclas de cualquier picker.
pub const PICKER_CONTEXT: &str = "Picker";
/// Lo que se agrega al contexto si el picker abre en un split.
pub const SPLITS_CONTEXT: &str = "splits";

macro_rules! picker_actions {
    ($($(#[doc = $doc:literal])* $action:ident = $name:literal;)*) => {
        $(
            $(#[doc = $doc])*
            #[derive(Clone, PartialEq, Eq, Default, Debug, Action)]
            #[action(namespace = picker, name = $name)]
            pub struct $action;
        )*
    };
}

picker_actions! {
    SelectNext = "select_next";
    SelectPrevious = "select_previous";
    /// Elige la fila marcada.
    Confirm = "confirm";
    /// Abre lo elegido en un panel nuevo a la derecha.
    ConfirmSplitRight = "confirm_split_right";
    /// Abre lo elegido en un panel nuevo abajo.
    ConfirmSplitDown = "confirm_split_down";
    /// Cierra el picker sin elegir.
    Dismiss = "dismiss";
    DeleteBackward = "delete_backward";
    DeleteForward = "delete_forward";
    DeleteWordBackward = "delete_word_backward";
    DeleteToStart = "delete_to_start";
    MoveLeft = "move_left";
    MoveRight = "move_right";
    MoveToStart = "move_to_start";
    MoveToEnd = "move_to_end";
}

pub fn bind_keys(cx: &mut App) {
    let picker = Some(PICKER_CONTEXT);
    let splits = format!("{PICKER_CONTEXT} && {SPLITS_CONTEXT}");
    cx.bind_keys([
        KeyBinding::new("down", SelectNext, picker),
        KeyBinding::new("ctrl-n", SelectNext, picker),
        KeyBinding::new("ctrl-j", SelectNext, picker),
        KeyBinding::new("up", SelectPrevious, picker),
        KeyBinding::new("ctrl-p", SelectPrevious, picker),
        KeyBinding::new("ctrl-k", SelectPrevious, picker),
        KeyBinding::new("enter", Confirm, picker),
        KeyBinding::new("escape", Dismiss, picker),
        KeyBinding::new("ctrl-c", Dismiss, picker),
        KeyBinding::new("backspace", DeleteBackward, picker),
        KeyBinding::new("delete", DeleteForward, picker),
        KeyBinding::new("ctrl-w", DeleteWordBackward, picker),
        KeyBinding::new("ctrl-backspace", DeleteWordBackward, picker),
        KeyBinding::new("ctrl-u", DeleteToStart, picker),
        KeyBinding::new("left", MoveLeft, picker),
        KeyBinding::new("right", MoveRight, picker),
        KeyBinding::new("home", MoveToStart, picker),
        KeyBinding::new("ctrl-a", MoveToStart, picker),
        KeyBinding::new("end", MoveToEnd, picker),
        KeyBinding::new("ctrl-e", MoveToEnd, picker),
        KeyBinding::new("ctrl-v", ConfirmSplitRight, Some(&splits)),
        KeyBinding::new("ctrl-s", ConfirmSplitDown, Some(&splits)),
    ]);
}
