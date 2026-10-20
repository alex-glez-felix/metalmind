//! El keymap de toda la app: los atajos del motor vim y los de cada vista,
//! en una sola lista. De ella salen la tabla del motor y los atajos de la
//! ventana.

use metalmind_vim::{ActionName, Bound, Context as VimContext, Key};

use crate::command::ex_commands;
use crate::{Action, Keystroke, View, defaults};

/// Dónde vale un atajo y con qué teclas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Keys {
    /// Las resuelve el motor vim en ese modo, con su count, lo pendiente y
    /// la cancelación de vim (`d ctrl-w` cancela el `d`).
    Vim(VimContext, Vec<Key>),
    /// Las resuelve la ventana con esa vista en el foco.
    View(View, Vec<Keystroke>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub keys: Keys,
    pub action: Action,
}

impl Binding {
    /// Las teclas como se muestran junto a la acción: `ctrl-w v`, `g g`.
    #[must_use]
    pub fn label(&self) -> String {
        let keys: Vec<String> = match &self.keys {
            Keys::Vim(_, keys) => keys.iter().map(ToString::to_string).collect(),
            Keys::View(_, keys) => keys.iter().map(ToString::to_string).collect(),
        };
        keys.join(" ")
    }
}

/// Ante dos atajos iguales gana el último, así una config de usuario podrá
/// sobreescribir los defaults.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keymap {
    bindings: Vec<Binding>,
}

impl Default for Keymap {
    /// Las teclas de edición de vim y los atajos de la ventana.
    fn default() -> Self {
        let vim = metalmind_vim::Keymap::vim();
        // El keymap de vim por defecto solo trae acciones del motor.
        let editing = vim
            .bindings()
            .iter()
            .filter_map(|binding| match binding.bound {
                Bound::Engine(action) => Some(Binding {
                    keys: Keys::Vim(binding.context, binding.keys.clone()),
                    action: Action::Vim(action),
                }),
                Bound::Host(_) => None,
            });
        Self {
            bindings: editing.chain(defaults::bindings()).collect(),
        }
    }
}

impl Keymap {
    #[must_use]
    pub fn bindings(&self) -> &[Binding] {
        &self.bindings
    }

    /// La tabla del motor: sus atajos, con los de la ventana por nombre, y
    /// los comandos de `:` de la ventana.
    #[must_use]
    pub fn engine(&self) -> metalmind_vim::Keymap {
        let bindings = self.bindings.iter().filter_map(|binding| {
            let Keys::Vim(context, keys) = &binding.keys else {
                return None;
            };
            let bound = match binding.action {
                Action::Vim(action) => Bound::Engine(action),
                action => Bound::Host(ActionName::new(action.name())),
            };
            Some(metalmind_vim::Binding {
                context: *context,
                keys: keys.clone(),
                bound,
            })
        });
        metalmind_vim::Keymap::new(bindings, ex_commands())
    }

    /// Los atajos que resuelve la ventana.
    pub fn view_bindings(&self) -> impl Iterator<Item = (View, &[Keystroke], Action)> {
        self.bindings
            .iter()
            .filter_map(|binding| match &binding.keys {
                Keys::View(view, keys) => Some((*view, keys.as_slice(), binding.action)),
                Keys::Vim(..) => None,
            })
    }

    /// El atajo que se muestra junto a `action`: el último de una vista en
    /// `active`, que es el que gana, o si no el primero del motor.
    #[must_use]
    pub fn keys_for(&self, action: Action, active: impl Fn(View) -> bool) -> Option<String> {
        let mine = self
            .bindings
            .iter()
            .filter(|binding| binding.action == action);
        let in_view = mine
            .clone()
            .filter(|binding| matches!(binding.keys, Keys::View(view, _) if active(view)))
            .last();
        let in_engine = || {
            mine.clone()
                .find(|binding| matches!(binding.keys, Keys::Vim(..)))
        };
        in_view.or_else(in_engine).map(Binding::label)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use metalmind_vim::{Mode, Operator, VisualKind};

    use super::*;
    use crate::{Command, KeyName, PanelAction};

    const MODES: [Mode; 14] = [
        Mode::Normal,
        Mode::Insert,
        Mode::CommandLine,
        Mode::OperatorPending(Operator::Delete),
        Mode::OperatorPending(Operator::Change),
        Mode::OperatorPending(Operator::Yank),
        Mode::OperatorPending(Operator::Lowercase),
        Mode::OperatorPending(Operator::Uppercase),
        Mode::OperatorPending(Operator::ToggleCase),
        Mode::OperatorPending(Operator::Indent),
        Mode::OperatorPending(Operator::Outdent),
        Mode::Visual(VisualKind::Char),
        Mode::Visual(VisualKind::Line),
        Mode::Visual(VisualKind::Block),
    ];

    fn overlap(a: VimContext, b: VimContext) -> bool {
        MODES.iter().any(|&mode| a.matches(mode) && b.matches(mode))
    }

    /// Una secuencia que fuera exacta y a la vez prefijo de otra necesitaría
    /// el timeout de vim para decidir; con los atajos de la ventana, la
    /// tabla del motor tampoco debe tener ninguna.
    #[test]
    fn la_tabla_del_motor_no_es_ambigua() {
        let engine = Keymap::default().engine();
        let bindings = engine.bindings();
        for (i, binding) in bindings.iter().enumerate() {
            for other in &bindings[i + 1..] {
                if !overlap(binding.context, other.context) {
                    continue;
                }
                let (keys, other_keys) = (&binding.keys, &other.keys);
                let shorter = keys.len().min(other_keys.len());
                assert!(
                    keys[..shorter] != other_keys[..shorter] || keys.len() == other_keys.len(),
                    "{keys:?} y {other_keys:?} se pisan"
                );
                assert!(keys != other_keys, "{keys:?} está dos veces");
            }
        }
    }

    #[test]
    fn ningun_atajo_de_la_ventana_esta_dos_veces() {
        let keymap = Keymap::default();
        let mut seen = HashSet::new();
        for (view, keys, action) in keymap.view_bindings() {
            assert!(
                seen.insert((view, keys.to_vec())),
                "{keys:?} en {view:?}: {action:?}"
            );
        }
    }

    /// Lo que no es del motor le llega por nombre; lo suyo, como acción.
    #[test]
    fn el_motor_recibe_las_acciones_ajenas_por_nombre() {
        let engine = Keymap::default().engine();
        let bound = |keys: &[Key]| {
            engine
                .bindings()
                .iter()
                .find(|binding| binding.context == VimContext::Normal && binding.keys == keys)
                .map(|binding| binding.bound)
        };
        assert_eq!(
            bound(&[Key::Char(' '), Key::Char('f')]),
            Some(Bound::Host(ActionName::new("workspace::open_finder")))
        );
        assert_eq!(
            bound(&[Key::Char('x')]),
            Some(Bound::Engine(metalmind_vim::Action::DeleteRight))
        );
    }

    /// `space e` y `ctrl-w l` se declaran una vez y valen en el editor y en
    /// el árbol.
    #[test]
    fn space_e_y_ctrl_w_l_valen_en_el_editor_y_en_el_arbol() {
        let keymap = Keymap::default();
        let space_e = |binding: &&Binding| match &binding.keys {
            Keys::Vim(VimContext::Normal, keys) => keys == &[Key::Char(' '), Key::Char('e')],
            Keys::View(View::FileTree, keys) => {
                keys == &[
                    Keystroke::plain(KeyName::Char(' ')),
                    Keystroke::plain(KeyName::Char('e')),
                ]
            }
            _ => false,
        };
        let toggles: Vec<Action> = keymap
            .bindings()
            .iter()
            .filter(space_e)
            .map(|b| b.action)
            .collect();
        assert_eq!(
            toggles,
            [Command::ToggleFolders.into(), Command::ToggleFolders.into()]
        );
        let tree = |view| view == View::FileTree;
        assert_eq!(
            keymap
                .keys_for(PanelAction::FocusEditor.into(), tree)
                .as_deref(),
            Some("ctrl-w ctrl-l")
        );
        assert_eq!(
            keymap
                .keys_for(Command::FocusRight.into(), |_| false)
                .as_deref(),
            Some("ctrl-w l")
        );
    }

    #[test]
    fn el_atajo_de_la_vista_enfocada_gana_al_del_motor() {
        let keymap = Keymap::default();
        let palette = Command::OpenCommandPalette.into();
        assert_eq!(
            keymap
                .keys_for(palette, |view| view == View::Editor)
                .as_deref(),
            Some("ctrl-shift-p")
        );
        assert_eq!(
            keymap.keys_for(palette, |_| false).as_deref(),
            Some("space p")
        );
        let last = metalmind_vim::Action::Motion(metalmind_vim::Motion::DocumentEnd);
        assert_eq!(
            keymap.keys_for(Action::Vim(last), |_| true).as_deref(),
            Some("G")
        );
        assert_eq!(
            keymap
                .keys_for(PanelAction::SelectLast.into(), |view| view
                    == View::FileTree)
                .as_deref(),
            Some("G")
        );
        assert_eq!(keymap.keys_for(Command::Save.into(), |_| true), None);
    }

    /// Cada acción de la ventana que se teclea en el editor tiene su
    /// nombre en el registro.
    #[test]
    fn los_nombres_que_devuelve_el_motor_son_del_registro() {
        for binding in Keymap::default().engine().bindings() {
            if let Bound::Host(name) = binding.bound {
                assert!(Action::from_name(name.as_str()).is_some(), "{name}");
            }
        }
    }
}
