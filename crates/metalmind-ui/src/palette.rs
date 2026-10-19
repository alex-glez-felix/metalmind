//! La paleta de comandos: un picker sobre todas las acciones con nombre:
//! las del registro (el motor y los comandos) y las demás acciones de la
//! ventana (las que GPUI tiene disponibles en el editor enfocado). Una
//! acción nueva aparece sola.

use gpui_kit::{Action as GpuiAction, App, Window};
use metalmind_keymap::{Action, Command, Keymap};

use crate::actions::Dispatch;
use crate::picker::PickerItem;

/// Quién ejecuta la acción.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// Una acción del registro, sin argumento: se despacha en el editor como
    /// [`Dispatch`].
    Registry(Action),
    /// Otra acción de GPUI, por su nombre: se despacha en el editor.
    Window(&'static str),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// `editor: save` para `editor::save`, como la paleta de Zed.
    pub label: String,
    pub target: Target,
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
/// por nombre. Las acciones de GPUI que piden argumentos no se pueden
/// construir sin ellos y quedan para `:`.
#[must_use]
pub fn entries(window: &Window, keymap: &Keymap, cx: &App) -> Vec<Entry> {
    let window_keys = |action: &dyn GpuiAction| {
        let binding = window.highest_precedence_binding_for_action(action)?;
        let strokes: Vec<String> = binding
            .keystrokes()
            .iter()
            .map(ToString::to_string)
            .collect();
        Some(strokes.join(" "))
    };
    let windowed = window
        .available_actions(cx)
        .into_iter()
        .map(|action| Entry {
            label: label(action.name()),
            target: Target::Window(action.name()),
            keys: window_keys(action.as_ref()),
        });
    let mut entries: Vec<Entry> = registry_entries(keymap, window_keys)
        .chain(windowed)
        .collect();
    entries.sort_by(|a, b| a.label.cmp(&b.label));
    entries.dedup_by(|a, b| a.label == b.label);
    entries
}

/// Las acciones del registro que se corren sin argumento, con el atajo de
/// la ventana o, si no tiene, el del motor.
fn registry_entries<'a>(
    keymap: &'a Keymap,
    window_keys: impl Fn(&dyn GpuiAction) -> Option<String> + 'a,
) -> impl Iterator<Item = Entry> + 'a {
    Action::all()
        .filter(|&action| !needs_argument_in_palette(action))
        .map(move |action| Entry {
            label: label(action.name()),
            target: Target::Registry(action),
            keys: window_keys(&Dispatch(action.into())).or_else(|| keymap.keys_for(action)),
        })
}

/// Sin argumento, `:w!` pisa el disco, `:e` relee y `:e!` descarta los
/// cambios, cosas que su nombre no avisa; `:set` se queja.
fn needs_argument_in_palette(action: Action) -> bool {
    matches!(
        action,
        Action::Command(Command::ForceSave | Command::Edit | Command::ForceEdit | Command::Set)
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

    use metalmind_vim::{Action as VimAction, Motion};

    use super::*;

    #[test]
    fn el_nombre_se_lee_como_en_zed() {
        assert_eq!(label("vim::next_word_start"), "vim: next word start");
        assert_eq!(label("workspace::open_finder"), "workspace: open finder");
        assert_eq!(label("a::b::c_d"), "a b: c d");
        assert_eq!(label("suelta"), "suelta");
    }

    fn entry(entries: &[Entry], action: impl Into<Action>) -> Option<&Entry> {
        let target = Target::Registry(action.into());
        entries.iter().find(|entry| entry.target == target)
    }

    #[test]
    fn las_acciones_del_motor_llevan_su_atajo() {
        let keymap = Keymap::default();
        let entries: Vec<Entry> = registry_entries(&keymap, |_| None).collect();
        let start = Action::Vim(VimAction::Motion(Motion::DocumentStart));
        let start = entry(&entries, start).unwrap();
        assert_eq!(start.label, "vim: start of document");
        assert_eq!(start.keys.as_deref(), Some("g g"));
        assert_eq!(
            entry(&entries, Command::OpenFinder)
                .unwrap()
                .keys
                .as_deref(),
            Some("space f")
        );
        let labels: HashSet<&str> = entries.iter().map(|entry| entry.label.as_str()).collect();
        assert_eq!(labels.len(), entries.len());
    }

    /// Como antes de que la paleta leyera el registro: todos los comandos
    /// menos los que sin argumento hacen algo que su nombre no dice.
    #[test]
    fn lista_los_comandos_que_corren_sin_argumento() {
        let keymap = Keymap::default();
        let entries: Vec<Entry> = registry_entries(&keymap, |_| None).collect();
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
        ];
        let expected: Vec<Command> = Command::ALL
            .iter()
            .copied()
            .filter(|command| !typed_only.contains(command))
            .collect();
        assert_eq!(listed, expected);
    }

    #[test]
    fn el_atajo_de_la_ventana_gana_al_del_motor() {
        let keymap = Keymap::default();
        let palette = Dispatch(Command::OpenCommandPalette.into());
        let entries: Vec<Entry> = registry_entries(&keymap, |action| {
            action
                .partial_eq(&palette)
                .then(|| "ctrl-shift-p".to_owned())
        })
        .collect();
        let keys = |command| entry(&entries, command).unwrap().keys.as_deref();
        assert_eq!(keys(Command::OpenCommandPalette), Some("ctrl-shift-p"));
        assert_eq!(keys(Command::OpenFinder), Some("space f"));
    }
}
