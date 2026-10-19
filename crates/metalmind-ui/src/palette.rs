//! La paleta de comandos: un picker sobre todas las acciones con nombre:
//! los comandos, las demás acciones de la ventana (las que GPUI tiene
//! disponibles en el editor enfocado) y las del motor vim. Una acción nueva
//! aparece sola.

use gpui_kit::{Action, App, Window};
use metalmind_vim::{Action as VimAction, Command, Key, Keymap};

use crate::actions::RunCommand;
use crate::picker::PickerItem;

/// Quién ejecuta la acción.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// Un comando, sin argumento: se despacha en el editor como
    /// [`RunCommand`].
    Command(Command),
    /// Otra acción de GPUI, por su nombre: se despacha en el editor.
    Window(&'static str),
    /// Una acción del motor, sobre el panel enfocado.
    Vim(VimAction),
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
    let window_keys = |action: &dyn Action| {
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
    let mut entries: Vec<Entry> = command_entries(keymap, window_keys)
        .chain(windowed)
        .chain(vim_entries(keymap))
        .collect();
    entries.sort_by(|a, b| a.label.cmp(&b.label));
    entries.dedup_by(|a, b| a.label == b.label);
    entries
}

/// Los comandos que se corren sin argumento, con el atajo de la ventana o,
/// si no tiene, el de vim.
fn command_entries<'a>(
    keymap: &'a Keymap,
    window_keys: impl Fn(&dyn Action) -> Option<String> + 'a,
) -> impl Iterator<Item = Entry> + 'a {
    Command::ALL
        .iter()
        .filter(|&&command| !needs_argument_in_palette(command))
        .map(move |&command| Entry {
            label: label(command.name()),
            target: Target::Command(command),
            keys: window_keys(&RunCommand::from(command))
                .or_else(|| vim_keys(keymap, VimAction::Command(command))),
        })
}

/// Sin argumento, `:w!` pisa el disco, `:e` relee y `:e!` descarta los
/// cambios, cosas que su nombre no avisa; `:set` se queja.
fn needs_argument_in_palette(command: Command) -> bool {
    matches!(
        command,
        Command::ForceSave | Command::Edit | Command::ForceEdit | Command::Set
    )
}

/// Las acciones del motor que no son comandos: esos ya son acciones de la
/// ventana.
fn vim_entries(keymap: &Keymap) -> impl Iterator<Item = Entry> + '_ {
    VimAction::ALL
        .iter()
        .filter(|action| !matches!(action, VimAction::Command(_)))
        .map(|&action| Entry {
            label: label(action.name()),
            target: Target::Vim(action),
            keys: vim_keys(keymap, action),
        })
}

fn vim_keys(keymap: &Keymap, action: VimAction) -> Option<String> {
    let keys = keymap.keys_for(action)?;
    let keys: Vec<String> = keys.iter().map(Key::to_string).collect();
    Some(keys.join(" "))
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

    use super::*;

    #[test]
    fn el_nombre_se_lee_como_en_zed() {
        assert_eq!(label("vim::next_word_start"), "vim: next word start");
        assert_eq!(label("workspace::open_finder"), "workspace: open finder");
        assert_eq!(label("a::b::c_d"), "a b: c d");
        assert_eq!(label("suelta"), "suelta");
    }

    #[test]
    fn las_acciones_de_vim_llevan_su_atajo_y_no_repiten_comandos() {
        let keymap = Keymap::vim();
        let entries: Vec<Entry> = vim_entries(&keymap).collect();
        let down = entries
            .iter()
            .find(|entry| {
                entry.target == Target::Vim(VimAction::Motion(metalmind_vim::Motion::DocumentStart))
            })
            .unwrap();
        assert_eq!(down.label, "vim: start of document");
        assert_eq!(down.keys.as_deref(), Some("g g"));
        assert!(
            entries
                .iter()
                .all(|entry| !matches!(entry.target, Target::Vim(VimAction::Command(_))))
        );
        let labels: HashSet<&str> = entries.iter().map(|entry| entry.label.as_str()).collect();
        assert_eq!(labels.len(), entries.len());
    }

    #[test]
    fn el_atajo_de_un_comando_sale_del_keymap_de_vim() {
        let keymap = Keymap::vim();
        let keys = vim_keys(&keymap, VimAction::Command(Command::OpenFinder));
        assert_eq!(keys.as_deref(), Some("space f"));
    }

    #[test]
    fn lista_los_comandos_que_corren_sin_argumento() {
        let keymap = Keymap::vim();
        let listed: Vec<Target> = command_entries(&keymap, |_| None)
            .map(|entry| entry.target)
            .collect();
        let typed_only = [
            Command::ForceSave,
            Command::Edit,
            Command::ForceEdit,
            Command::Set,
        ];
        let expected: Vec<Target> = Command::ALL
            .iter()
            .filter(|command| !typed_only.contains(command))
            .map(|&command| Target::Command(command))
            .collect();
        assert_eq!(listed, expected);
    }

    #[test]
    fn el_atajo_de_la_ventana_gana_al_de_vim() {
        let keymap = Keymap::vim();
        let palette = RunCommand::from(Command::OpenCommandPalette);
        let entries: Vec<Entry> = command_entries(&keymap, |action| {
            action
                .partial_eq(&palette)
                .then(|| "ctrl-shift-p".to_owned())
        })
        .collect();
        let keys = |command| {
            let entry = entries
                .iter()
                .find(|entry| entry.target == Target::Command(command))
                .unwrap();
            entry.keys.as_deref()
        };
        assert_eq!(keys(Command::OpenCommandPalette), Some("ctrl-shift-p"));
        assert_eq!(keys(Command::OpenFinder), Some("space f"));
    }
}
