//! Acciones de la ventana que no son de vim. Los atajos, la paleta y `:`
//! solo las invocan por su nombre.

use gpui_kit::{Action, App, KeyBinding};
use metalmind_vim::{Command, Invocation};

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
    SaveAndQuit = editor::"save_and_quit";
    /// Cierra el panel; el último cierra la ventana si ningún buffer tiene
    /// cambios.
    Quit = editor::"quit";
    /// Cierra el panel; el último cierra la ventana aunque se pierdan
    /// cambios.
    ForceQuit = editor::"force_quit";
    /// Guarda todos los buffers con cambios, también los ocultos.
    SaveAll = workspace::"save_all";
    /// Cierra la ventana si ningún buffer tiene cambios.
    QuitAll = workspace::"quit_all";
    /// Cierra la ventana aunque se pierdan cambios.
    ForceQuitAll = workspace::"force_quit_all";
    /// Guarda todos los buffers con cambios y cierra la ventana.
    SaveAllAndQuit = workspace::"save_all_and_quit";
    SplitRight = pane::"split_right";
    SplitDown = pane::"split_down";
    FocusLeft = pane::"focus_left";
    FocusDown = pane::"focus_down";
    FocusUp = pane::"focus_up";
    FocusRight = pane::"focus_right";
    FocusNext = pane::"focus_next";
    /// Cierra los demás paneles; sus buffers quedan ocultos.
    Only = pane::"only";
    /// Busca una nota del vault por su ruta.
    OpenFinder = workspace::"open_finder";
    /// Busca texto en las notas del vault y abre la nota en esa línea.
    OpenTextSearch = workspace::"open_text_search";
    /// Busca y ejecuta cualquier acción con nombre.
    OpenCommandPalette = workspace::"open_command_palette";
    /// Muestra el panel lateral en carpetas, o lo esconde si ya las muestra.
    ToggleFolders = panel::"toggle_folders";
    /// Muestra el panel lateral en tags, o lo esconde si ya los muestra.
    ToggleTags = panel::"toggle_tags";
    /// Abre la nota del link bajo el cursor, en el heading o el bloque que
    /// nombre.
    FollowLink = editor::"follow_link";
}

/// `:w ruta`: le da ese archivo a un buffer sin nombre o escribe una copia.
/// Aparte de [`Save`], que no lleva datos y así la paleta la puede armar.
#[derive(Clone, PartialEq, Eq, Default, Debug, Action)]
#[action(namespace = editor, name = "save_as", no_json)]
pub struct SaveAs {
    pub path: String,
}

impl CommandAction for SaveAs {
    fn invocation(&self) -> Invocation {
        Invocation {
            command: Command::Save,
            argument: Some(self.path.clone()),
        }
    }
}

/// `:wq ruta`: como [`SaveAs`], y después cierra el panel.
#[derive(Clone, PartialEq, Eq, Default, Debug, Action)]
#[action(namespace = editor, name = "save_as_and_quit", no_json)]
pub struct SaveAsAndQuit {
    pub path: String,
}

impl CommandAction for SaveAsAndQuit {
    fn invocation(&self) -> Invocation {
        Invocation {
            command: Command::SaveAndQuit,
            argument: Some(self.path.clone()),
        }
    }
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

/// `:w! [ruta]`: guarda aunque el archivo haya cambiado en disco, o pisa
/// la ruta aunque exista.
#[derive(Clone, PartialEq, Eq, Default, Debug, Action)]
#[action(namespace = editor, name = "force_save", no_json)]
pub struct ForceSave {
    pub path: Option<String>,
}

impl CommandAction for ForceSave {
    fn invocation(&self) -> Invocation {
        Invocation {
            command: Command::ForceSave,
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
        Command::Save => match argument {
            Some(path) => Box::new(SaveAs { path }),
            None => Box::new(Save),
        },
        Command::ForceSave => Box::new(ForceSave { path: argument }),
        Command::Quit => Box::new(Quit),
        Command::ForceQuit => Box::new(ForceQuit),
        Command::SaveAndQuit => match argument {
            Some(path) => Box::new(SaveAsAndQuit { path }),
            None => Box::new(SaveAndQuit),
        },
        Command::SaveAll => Box::new(SaveAll),
        Command::QuitAll => Box::new(QuitAll),
        Command::ForceQuitAll => Box::new(ForceQuitAll),
        Command::SaveAllAndQuit => Box::new(SaveAllAndQuit),
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
        Command::OpenFinder => Box::new(OpenFinder),
        Command::OpenTextSearch => Box::new(OpenTextSearch),
        Command::OpenCommandPalette => Box::new(OpenCommandPalette),
        Command::ToggleFolders => Box::new(ToggleFolders),
        Command::ToggleTags => Box::new(ToggleTags),
        Command::FollowLink => Box::new(FollowLink),
    }
}

/// `ctrl-shift-*` no lo usa vim, así que queda para el portapapeles del
/// sistema y para la paleta, con el atajo de Zed.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-shift-c", Copy, Some(EDITOR_CONTEXT)),
        KeyBinding::new("ctrl-shift-v", Paste, Some(EDITOR_CONTEXT)),
        KeyBinding::new("ctrl-shift-p", OpenCommandPalette, Some(EDITOR_CONTEXT)),
    ]);
    crate::sidebar::bind_keys(cx);
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

    /// `editor::save` no lleva datos, para que la paleta la arme; la ruta
    /// de `:w ruta` y `:wq ruta` va en `editor::save_as…`.
    #[test]
    fn la_ruta_de_w_viaja_en_otra_accion() {
        for (command, name) in [
            (Command::Save, "editor::save_as"),
            (Command::SaveAndQuit, "editor::save_as_and_quit"),
        ] {
            let invocation = Invocation {
                command,
                argument: Some("nueva.md".to_owned()),
            };
            let action = for_invocation(invocation.clone());
            assert_eq!(action.name(), name);
            let back = action
                .as_any()
                .downcast_ref::<SaveAs>()
                .map(SaveAs::invocation)
                .or_else(|| {
                    let quit = action.as_any().downcast_ref::<SaveAsAndQuit>()?;
                    Some(quit.invocation())
                });
            assert_eq!(back, Some(invocation));
        }
        assert_eq!(for_invocation(Command::Save.into()).name(), "editor::save");
    }
}
