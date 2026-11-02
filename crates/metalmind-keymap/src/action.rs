//! Todas las acciones con nombre de Metalmind, las del motor vim y las de
//! la ventana, en un solo registro.

use metalmind_vim::Action as VimAction;

use crate::{
    ChangePreviewAction, Command, ContextAction, EditorAction, ListAction, PanelAction,
    PickerAction, WindowCommand,
};

/// Una acción con nombre, de cualquier parte de la app.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Del motor, sobre el panel enfocado.
    Vim(VimAction),
    /// Del modelo de la ventana, que decide qué pasa.
    Command(Command),
    /// De las vistas de la ventana: el modelo solo corta la macro.
    Window(WindowCommand),
    Editor(EditorAction),
    Panel(PanelAction),
    List(ListAction),
    Context(ContextAction),
    Picker(PickerAction),
    ChangePreview(ChangePreviewAction),
}

/// Quién atiende una acción: solo se puede invocar con su vista en el foco.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Owner {
    /// La raíz de la ventana, que siempre está: los comandos y el motor.
    Window,
    Editor,
    Panel,
    List,
    Context,
    Picker,
    ChangePreview,
}

impl Action {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Vim(action) => action.name(),
            Self::Command(command) => command.name(),
            Self::Window(command) => command.name(),
            Self::Editor(action) => action.name(),
            Self::Panel(action) => action.name(),
            Self::List(action) => action.name(),
            Self::Context(action) => action.name(),
            Self::Picker(action) => action.name(),
            Self::ChangePreview(action) => action.name(),
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::all().find(|action| action.name() == name)
    }

    /// Cada acción una vez, en el orden de su `enum`.
    pub fn all() -> impl Iterator<Item = Self> {
        let vim = VimAction::ALL.iter().copied().map(Self::Vim);
        let commands = Command::ALL.iter().copied().map(Self::Command);
        let window = WindowCommand::ALL.iter().copied().map(Self::Window);
        let editor = EditorAction::ALL.iter().copied().map(Self::Editor);
        let panel = PanelAction::ALL.iter().copied().map(Self::Panel);
        let list = ListAction::ALL.iter().copied().map(Self::List);
        let context = ContextAction::ALL.iter().copied().map(Self::Context);
        let picker = PickerAction::ALL.iter().copied().map(Self::Picker);
        let change_preview = ChangePreviewAction::ALL
            .iter()
            .copied()
            .map(Self::ChangePreview);
        vim.chain(commands)
            .chain(window)
            .chain(editor)
            .chain(panel)
            .chain(list)
            .chain(context)
            .chain(picker)
            .chain(change_preview)
    }

    #[must_use]
    pub fn owner(self) -> Owner {
        match self {
            Self::Vim(_) | Self::Command(_) | Self::Window(_) => Owner::Window,
            Self::Editor(_) => Owner::Editor,
            Self::Panel(_) => Owner::Panel,
            Self::List(_) => Owner::List,
            Self::Context(_) => Owner::Context,
            Self::Picker(_) => Owner::Picker,
            Self::ChangePreview(_) => Owner::ChangePreview,
        }
    }
}

impl From<Command> for Action {
    fn from(command: Command) -> Self {
        Self::Command(command)
    }
}

impl From<WindowCommand> for Action {
    fn from(command: WindowCommand) -> Self {
        Self::Window(command)
    }
}

impl From<EditorAction> for Action {
    fn from(action: EditorAction) -> Self {
        Self::Editor(action)
    }
}

impl From<PanelAction> for Action {
    fn from(action: PanelAction) -> Self {
        Self::Panel(action)
    }
}

impl From<ListAction> for Action {
    fn from(action: ListAction) -> Self {
        Self::List(action)
    }
}

impl From<ContextAction> for Action {
    fn from(action: ContextAction) -> Self {
        Self::Context(action)
    }
}

impl From<PickerAction> for Action {
    fn from(action: PickerAction) -> Self {
        Self::Picker(action)
    }
}

impl From<ChangePreviewAction> for Action {
    fn from(action: ChangePreviewAction) -> Self {
        Self::ChangePreview(action)
    }
}

/// Una acción con lo que se le escribió tras su nombre en `:`, como
/// `:e notas/a.md`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invocation {
    pub action: Action,
    pub argument: Option<String>,
}

/// Un nombre que no es de ninguna acción.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("no hay ninguna acción que se llame {0}")]
pub struct UnknownAction(pub String);

impl Invocation {
    /// La acción que se llama `name`, con `argument`.
    ///
    /// # Errors
    /// [`UnknownAction`] si ninguna se llama así.
    pub fn resolve(name: &str, argument: Option<String>) -> Result<Self, UnknownAction> {
        let action = Action::from_name(name).ok_or_else(|| UnknownAction(name.to_owned()))?;
        Ok(Self { action, argument })
    }
}

impl From<Action> for Invocation {
    fn from(action: Action) -> Self {
        Self {
            action,
            argument: None,
        }
    }
}

/// Cada `enum` de acciones, sin argumento.
macro_rules! invocations {
    ($($action:ty),*) => {
        $(
            impl From<$action> for Invocation {
                fn from(action: $action) -> Self {
                    Action::from(action).into()
                }
            }
        )*
    };
}

invocations!(
    Command,
    WindowCommand,
    EditorAction,
    PanelAction,
    ContextAction,
    PickerAction
);

impl TryFrom<metalmind_vim::Invocation> for Invocation {
    type Error = UnknownAction;

    fn try_from(invocation: metalmind_vim::Invocation) -> Result<Self, Self::Error> {
        Self::resolve(invocation.action.as_str(), invocation.argument)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn cada_nombre_es_de_una_sola_accion() {
        let names: Vec<&str> = Action::all().map(Action::name).collect();
        let unique: HashSet<&str> = names.iter().copied().collect();
        assert_eq!(unique.len(), names.len());
        for action in Action::all() {
            assert_eq!(Action::from_name(action.name()), Some(action));
        }
    }

    #[test]
    fn el_motor_no_tiene_comandos_de_la_ventana() {
        for &command in Command::ALL {
            assert_eq!(VimAction::from_name(command.name()), None, "{command:?}");
        }
    }

    #[test]
    fn un_nombre_desconocido_es_un_error() {
        assert_eq!(
            Invocation::resolve("editor::nada", None),
            Err(UnknownAction("editor::nada".to_owned()))
        );
        assert_eq!(
            Invocation::resolve("editor::save", Some("a.md".to_owned())),
            Ok(Invocation {
                action: Action::Command(Command::Save),
                argument: Some("a.md".to_owned()),
            })
        );
    }
}
