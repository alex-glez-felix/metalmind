//! El keymap de la app: los atajos que se teclean en el editor, de vim y de
//! la ventana, en una sola lista de la que sale la tabla del motor.

use metalmind_vim::{ActionName, Context as VimContext, Key, Target};

use crate::command::ex_commands;
use crate::{Action, defaults};

/// En `context`, la secuencia `keys` invoca `action`. Las resuelve el
/// motor vim, con su count, lo pendiente y la cancelación de vim (`d ctrl-w`
/// cancela el `d`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub context: VimContext,
    pub keys: Vec<Key>,
    pub action: Action,
}

impl Binding {
    /// Las teclas como se muestran junto a la acción: `ctrl-w v`, `g g`.
    #[must_use]
    pub fn label(&self) -> String {
        let keys: Vec<String> = self.keys.iter().map(ToString::to_string).collect();
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
            .filter_map(|binding| match binding.target {
                Target::Engine(action) => Some(Binding {
                    context: binding.context,
                    keys: binding.keys.clone(),
                    action: Action::Vim(action),
                }),
                Target::Host(_) => None,
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
        let bindings = self.bindings.iter().map(|binding| {
            let target = match binding.action {
                Action::Vim(action) => Target::Engine(action),
                action @ Action::Command(_) => Target::Host(ActionName::new(action.name())),
            };
            metalmind_vim::Binding {
                context: binding.context,
                keys: binding.keys.clone(),
                target,
            }
        });
        metalmind_vim::Keymap::new(bindings, ex_commands())
    }

    /// Las teclas del primer atajo de `action`, para mostrarlas junto a su
    /// nombre en la paleta.
    #[must_use]
    pub fn keys_for(&self, action: Action) -> Option<String> {
        self.bindings
            .iter()
            .find(|binding| binding.action == action)
            .map(Binding::label)
    }
}

#[cfg(test)]
mod tests {
    use metalmind_vim::{Mode, Operator, VisualKind};

    use super::*;
    use crate::Command;

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

    /// Lo que no es del motor le llega por nombre; lo suyo, como acción.
    #[test]
    fn el_motor_recibe_las_acciones_ajenas_por_nombre() {
        let engine = Keymap::default().engine();
        let target = |keys: &[Key]| {
            engine
                .bindings()
                .iter()
                .find(|binding| binding.context == VimContext::Normal && binding.keys == keys)
                .map(|binding| binding.target)
        };
        assert_eq!(
            target(&[Key::Char(' '), Key::Char('f')]),
            Some(Target::Host(ActionName::new("workspace::open_finder")))
        );
        assert_eq!(
            target(&[Key::Char('x')]),
            Some(Target::Engine(metalmind_vim::Action::DeleteRight))
        );
    }

    #[test]
    fn keys_for_da_el_primer_atajo() {
        let keymap = Keymap::default();
        assert_eq!(
            keymap.keys_for(Command::SplitRight.into()).as_deref(),
            Some("ctrl-w v")
        );
        assert_eq!(
            keymap.keys_for(Command::OpenFinder.into()).as_deref(),
            Some("space f")
        );
        let start = metalmind_vim::Action::Motion(metalmind_vim::Motion::DocumentStart);
        assert_eq!(keymap.keys_for(Action::Vim(start)).as_deref(), Some("g g"));
        assert_eq!(keymap.keys_for(Command::Save.into()), None);
    }

    /// Cada acción de la ventana que se teclea en el editor tiene su
    /// nombre en el registro.
    #[test]
    fn los_nombres_que_devuelve_el_motor_son_del_registro() {
        for binding in Keymap::default().engine().bindings() {
            if let Target::Host(name) = binding.target {
                assert!(Action::from_name(name.as_str()).is_some(), "{name}");
            }
        }
    }
}
