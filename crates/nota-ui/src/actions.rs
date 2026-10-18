//! Acciones de la ventana que no son de vim. Los atajos, la paleta y `:`
//! solo las invocan por su nombre.

use gpui_kit::{Action, App, KeyBinding};
use nota_vim::{Command, Invocation};

/// Contexto de teclas de la vista de editor.
pub const EDITOR_CONTEXT: &str = "Editor";

/// Una acción que ejecuta un comando de `:` o `ctrl-w` en el workspace.
pub trait CommandAction: Action {
    fn invocation(&self) -> Invocation;
}

/// Una acción sin datos por cada comando que no lleva argumento.
macro_rules! bare_commands {
    ($($(#[doc = $doc:literal])* $action:ident = $namespace:ident :: $name:literal;)*) => {
        $(
            $(#[doc = $doc])*
            #[derive(Clone, PartialEq, Eq, Default, Debug, Action)]
            #[action(namespace = $namespace, name = $name)]
            pub struct $action;

            impl CommandAction for $action {
                fn invocation(&self) -> Invocation {
                    Command::$action.into()
                }
            }
        )*
    };
}

bare_commands! {
    Save = editor::"save";
    /// Cierra el panel si no se pierden cambios; el último cierra la ventana.
    Quit = editor::"quit";
    /// Cierra el panel aunque se pierdan cambios.
    ForceQuit = editor::"force_quit";
    SaveAndQuit = editor::"save_and_quit";
    SplitRight = pane::"split_right";
    SplitDown = pane::"split_down";
    FocusLeft = pane::"focus_left";
    FocusDown = pane::"focus_down";
    FocusUp = pane::"focus_up";
    FocusRight = pane::"focus_right";
    FocusNext = pane::"focus_next";
    /// Cierra los demás paneles, salvo los que perderían cambios.
    Only = pane::"only";
}

/// Abre `path` en el panel, o relee su archivo si no hay ruta.
#[derive(Clone, PartialEq, Eq, Default, Debug, Action)]
#[action(namespace = editor, name = "edit", no_json)]
pub struct Edit {
    pub path: Option<String>,
}

impl CommandAction for Edit {
    fn invocation(&self) -> Invocation {
        Invocation {
            command: Command::Edit,
            argument: self.path.clone(),
        }
    }
}

/// Como [`Edit`], descartando los cambios.
#[derive(Clone, PartialEq, Eq, Default, Debug, Action)]
#[action(namespace = editor, name = "force_edit", no_json)]
pub struct ForceEdit {
    pub path: Option<String>,
}

impl CommandAction for ForceEdit {
    fn invocation(&self) -> Invocation {
        Invocation {
            command: Command::ForceEdit,
            argument: self.path.clone(),
        }
    }
}

/// `:set`: cambia opciones del panel enfocado, como sus números de línea.
#[derive(Clone, PartialEq, Eq, Default, Debug, Action)]
#[action(namespace = editor, name = "set", no_json)]
pub struct Set {
    pub options: Option<String>,
}

impl CommandAction for Set {
    fn invocation(&self) -> Invocation {
        Invocation {
            command: Command::Set,
            argument: self.options.clone(),
        }
    }
}

/// Copia la selección de Visual o del ratón, o la línea del cursor si no
/// hay ninguna.
#[derive(Clone, PartialEq, Eq, Default, Debug, Action)]
#[action(namespace = editor, name = "copy")]
pub struct Copy;

/// Pega en el cursor; en Visual, reemplaza la selección.
#[derive(Clone, PartialEq, Eq, Default, Debug, Action)]
#[action(namespace = editor, name = "paste")]
pub struct Paste;

/// La acción que invoca un comando de `:` o `ctrl-w`.
#[must_use]
pub fn for_invocation(invocation: Invocation) -> Box<dyn Action> {
    let argument = invocation.argument;
    match invocation.command {
        Command::Save => Box::new(Save),
        Command::Quit => Box::new(Quit),
        Command::ForceQuit => Box::new(ForceQuit),
        Command::SaveAndQuit => Box::new(SaveAndQuit),
        Command::Edit => Box::new(Edit { path: argument }),
        Command::ForceEdit => Box::new(ForceEdit { path: argument }),
        Command::SplitRight => Box::new(SplitRight),
        Command::SplitDown => Box::new(SplitDown),
        Command::FocusLeft => Box::new(FocusLeft),
        Command::FocusDown => Box::new(FocusDown),
        Command::FocusUp => Box::new(FocusUp),
        Command::FocusRight => Box::new(FocusRight),
        Command::FocusNext => Box::new(FocusNext),
        Command::Only => Box::new(Only),
        Command::Set => Box::new(Set { options: argument }),
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
            let action = for_invocation(command.into());
            assert_eq!(action.name(), command.name());
        }
    }

    #[test]
    fn la_ruta_de_e_viaja_en_la_accion() {
        let invocation = Invocation {
            command: Command::Edit,
            argument: Some("b.md".to_owned()),
        };
        let action = for_invocation(invocation.clone());
        assert_eq!(
            action.as_any().downcast_ref::<Edit>().map(Edit::invocation),
            Some(invocation)
        );
    }
}
