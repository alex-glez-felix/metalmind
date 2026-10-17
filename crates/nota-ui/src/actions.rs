//! Acciones del editor que no son de vim. Los atajos, la paleta y `:` solo
//! las invocan por su nombre.

use gpui_kit::{Action, App, KeyBinding};
use nota_vim::Command;

/// Contexto de teclas de la vista de editor.
pub const EDITOR_CONTEXT: &str = "Editor";

#[derive(Clone, PartialEq, Eq, Default, Debug, Action)]
#[action(namespace = editor, name = "save")]
pub struct Save;

/// Cierra si no hay cambios sin guardar.
#[derive(Clone, PartialEq, Eq, Default, Debug, Action)]
#[action(namespace = editor, name = "quit")]
pub struct Quit;

/// Cierra aunque haya cambios sin guardar.
#[derive(Clone, PartialEq, Eq, Default, Debug, Action)]
#[action(namespace = editor, name = "force_quit")]
pub struct ForceQuit;

#[derive(Clone, PartialEq, Eq, Default, Debug, Action)]
#[action(namespace = editor, name = "save_and_quit")]
pub struct SaveAndQuit;

/// Copia la selección del ratón, o la línea del cursor si no hay.
#[derive(Clone, PartialEq, Eq, Default, Debug, Action)]
#[action(namespace = editor, name = "copy")]
pub struct Copy;

#[derive(Clone, PartialEq, Eq, Default, Debug, Action)]
#[action(namespace = editor, name = "paste")]
pub struct Paste;

/// La acción que invoca un comando de `:`.
#[must_use]
pub fn for_command(command: Command) -> Box<dyn Action> {
    match command {
        Command::Save => Box::new(Save),
        Command::Quit => Box::new(Quit),
        Command::ForceQuit => Box::new(ForceQuit),
        Command::SaveAndQuit => Box::new(SaveAndQuit),
    }
}

/// `ctrl-shift-*` no lo usa vim, así que queda para el portapapeles del sistema.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-shift-c", Copy, Some(EDITOR_CONTEXT)),
        KeyBinding::new("ctrl-shift-v", Paste, Some(EDITOR_CONTEXT)),
    ]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cada_comando_invoca_la_accion_de_su_nombre() {
        for &command in Command::ALL {
            assert_eq!(for_command(command).name(), command.name());
        }
    }
}
