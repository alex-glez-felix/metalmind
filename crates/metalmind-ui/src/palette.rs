//! La paleta de comandos: un picker sobre las acciones del registro que
//! atiende la vista enfocada, con su atajo. Una acción nueva aparece sola.

use gpui_kit::{KeyBindingContextPredicate, KeyContext, Window};
use metalmind_keymap::{Action, Command, Keymap, Owner, View};

use crate::picker::PickerItem;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// `editor: save` para `editor::save`, como la paleta de Zed.
    pub label: String,
    pub action: Action,
    /// Su atajo, si tiene.
    pub keys: Option<String>,
}

impl AsRef<str> for Entry {
    fn as_ref(&self) -> &str {
        &self.label
    }
}

impl PickerItem for Entry {
    fn detail(&self) -> Option<&str> {
        self.keys.as_deref()
    }
}

/// Lo que lista la paleta abierta desde el elemento enfocado de `window`,
/// por nombre.
#[must_use]
pub fn entries(window: &Window, keymap: &Keymap) -> Vec<Entry> {
    let stack = window.context_stack();
    let active = |view: View| is_active(view, &stack);
    let focused = |owner| owns_focus(owner, active);
    listed(keymap, focused, active)
}

/// Las acciones de los dueños en `focused`, con el atajo que vale en las
/// vistas `active`, en orden por nombre.
fn listed(
    keymap: &Keymap,
    focused: impl Fn(Owner) -> bool,
    active: impl Fn(View) -> bool,
) -> Vec<Entry> {
    let mut entries: Vec<Entry> = Action::all()
        .filter(|&action| focused(action.owner()) && !needs_argument_in_palette(action))
        .map(|action| Entry {
            label: label(action.name()),
            action,
            keys: keymap.keys_for(action, &active),
        })
        .collect();
    entries.sort_by(|a, b| a.label.cmp(&b.label));
    entries
}

fn is_active(view: View, stack: &[KeyContext]) -> bool {
    KeyBindingContextPredicate::parse(view.predicate()).is_ok_and(|predicate| predicate.eval(stack))
}

fn owns_focus(owner: Owner, active: impl Fn(View) -> bool) -> bool {
    match owner {
        Owner::Window => true,
        Owner::Editor => active(View::Editor),
        Owner::Panel => [
            View::FileTree,
            View::FileTreeInput,
            View::FileTreeConfirm,
            View::FileTreeMenu,
        ]
        .into_iter()
        .any(active),
        Owner::Picker => active(View::Picker),
        Owner::TagChange => active(View::TagChange),
    }
}

/// Sin argumento, `:w!` pisa el disco, `:e` relee y `:e!` descarta los
/// cambios, cosas que su nombre no avisa; `:set` y los de tags se quejan.
fn needs_argument_in_palette(action: Action) -> bool {
    matches!(
        action,
        Action::Command(
            Command::ForceSave
                | Command::Edit
                | Command::ForceEdit
                | Command::Set
                | Command::RenameTag
                | Command::MergeTag
                | Command::EditTagAliases
                | Command::EditTagParents
                | Command::SetTagIcon
                | Command::SetTagTemplate
                | Command::OpenTagPage
        )
    )
}

/// `vim::next_word_start` → `vim: next word start`.
#[must_use]
pub fn label(name: &str) -> String {
    let (namespace, action) = name.rsplit_once("::").unwrap_or(("", name));
    let action = action.replace('_', " ");
    if namespace.is_empty() {
        action
    } else {
        format!("{}: {action}", namespace.replace("::", " "))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use metalmind_keymap::{EditorAction, PanelAction};
    use metalmind_vim::{Action as VimAction, Motion};

    use super::*;

    #[test]
    fn el_nombre_se_lee_como_en_zed() {
        assert_eq!(label("vim::next_word_start"), "vim: next word start");
        assert_eq!(label("workspace::open_finder"), "workspace: open finder");
        assert_eq!(label("a::b::c_d"), "a b: c d");
        assert_eq!(label("suelta"), "suelta");
    }

    fn from_editor() -> Vec<Entry> {
        let editor = |view| view == View::Editor;
        listed(
            &Keymap::default(),
            |owner| owns_focus(owner, editor),
            editor,
        )
    }

    fn entry(entries: &[Entry], action: impl Into<Action>) -> Option<&Entry> {
        let action = action.into();
        entries.iter().find(|entry| entry.action == action)
    }

    #[test]
    fn desde_el_editor_lista_el_motor_los_comandos_y_el_editor() {
        let entries = from_editor();
        let start = Action::Vim(VimAction::Motion(Motion::DocumentStart));
        let start = entry(&entries, start).unwrap();
        assert_eq!(start.label, "vim: start of document");
        assert_eq!(start.keys.as_deref(), Some("g g"));
        let copy = entry(&entries, EditorAction::Copy).unwrap();
        assert_eq!(copy.keys.as_deref(), Some("ctrl-shift-c"));
        assert_eq!(
            entry(&entries, Command::OpenFinder)
                .unwrap()
                .keys
                .as_deref(),
            Some("space f")
        );
        assert_eq!(
            entry(&entries, Command::OpenCommandPalette)
                .unwrap()
                .keys
                .as_deref(),
            Some("ctrl-shift-p")
        );
        assert!(entry(&entries, PanelAction::SelectNext).is_none());
        let table = entry(&entries, Action::Vim(VimAction::FormatTable)).unwrap();
        assert_eq!(table.label, "editor: format table");
        let labels: HashSet<&str> = entries.iter().map(|entry| entry.label.as_str()).collect();
        assert_eq!(labels.len(), entries.len());
    }

    /// Como antes de que la paleta leyera el registro: todos los comandos
    /// menos los que sin argumento hacen algo que su nombre no dice.
    #[test]
    fn lista_los_comandos_que_corren_sin_argumento() {
        let entries = from_editor();
        let listed: Vec<Command> = Command::ALL
            .iter()
            .copied()
            .filter(|&command| entry(&entries, command).is_some())
            .collect();
        let typed_only = [
            Command::ForceSave,
            Command::Edit,
            Command::ForceEdit,
            Command::Set,
            Command::RenameTag,
            Command::MergeTag,
            Command::EditTagAliases,
            Command::EditTagParents,
            Command::SetTagIcon,
            Command::SetTagTemplate,
            Command::OpenTagPage,
        ];
        let expected: Vec<Command> = Command::ALL
            .iter()
            .copied()
            .filter(|command| !typed_only.contains(command))
            .collect();
        assert_eq!(listed, expected);
    }

    #[test]
    fn desde_el_arbol_lista_el_panel_con_sus_teclas() {
        let tree = |view| view == View::FileTree;
        let entries = listed(&Keymap::default(), |owner| owns_focus(owner, tree), tree);
        let new_folder = entry(&entries, PanelAction::NewFolder).unwrap();
        assert_eq!(new_folder.keys.as_deref(), Some("A"));
        assert_eq!(
            entry(&entries, Command::ToggleFolders)
                .unwrap()
                .keys
                .as_deref(),
            Some("space e")
        );
        assert!(entry(&entries, EditorAction::Copy).is_none());
    }
}
