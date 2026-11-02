//! Lo que puede seguir a un atajo a medio teclear en el editor, para el
//! panel de `space` y los demás prefijos (#86).

use std::collections::HashSet;

use metalmind_vim::{Key, Mode};

use crate::{Action, Keymap, Keys};

/// Una acción que se alcanza desde lo tecleado, con las teclas que faltan
/// para cada atajo que la invoca, como `v` y `ctrl-v` tras `ctrl-w`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NextKey {
    pub keys: Vec<String>,
    pub action: Action,
}

impl Keymap {
    /// Lo que sigue a `typed` en `mode`, en el orden de la tabla: de dos
    /// atajos con las mismas teclas solo cuenta el que gana, el último.
    #[must_use]
    pub fn next_keys(&self, mode: Mode, typed: &[Key]) -> Vec<NextKey> {
        if typed.is_empty() {
            return Vec::new();
        }
        let mut seen = HashSet::new();
        let mut reachable: Vec<(&[Key], Action)> = self
            .bindings()
            .iter()
            .rev()
            .filter_map(|binding| match &binding.keys {
                Keys::Vim(context, keys)
                    if context.matches(mode)
                        && keys.len() > typed.len()
                        && keys.starts_with(typed) =>
                {
                    Some((&keys[typed.len()..], binding.action))
                }
                _ => None,
            })
            .filter(|(rest, _)| seen.insert(*rest))
            .collect();
        // Como en `lookup` del motor, un atajo que es el inicio de otro más
        // largo nunca se ejecuta: espera la tecla siguiente.
        let longer = reachable.clone();
        reachable.retain(|(rest, _)| {
            !longer
                .iter()
                .any(|(other, _)| other.len() > rest.len() && other.starts_with(rest))
        });
        reachable.reverse();
        group_by_action(reachable)
    }
}

fn group_by_action(reachable: Vec<(&[Key], Action)>) -> Vec<NextKey> {
    let mut grouped: Vec<NextKey> = Vec::new();
    for (rest, action) in reachable {
        let label = rest
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" ");
        match grouped.iter_mut().find(|next| next.action == action) {
            Some(next) => next.keys.push(label),
            None => grouped.push(NextKey {
                keys: vec![label],
                action,
            }),
        }
    }
    grouped
}

#[cfg(test)]
mod tests {
    use metalmind_vim::{Context as VimContext, Motion, Operator, TextObject};

    use super::*;
    use crate::{Binding, WindowCommand};

    fn next(keymap: &Keymap, mode: Mode, typed: &[Key]) -> Vec<(Vec<String>, &'static str)> {
        keymap
            .next_keys(mode, typed)
            .into_iter()
            .map(|next| (next.keys, next.action.name()))
            .collect()
    }

    fn keys(labels: &[&str]) -> Vec<String> {
        labels.iter().map(|&label| label.to_owned()).collect()
    }

    #[test]
    fn tras_space_salen_los_comandos_del_leader_en_su_orden() {
        let next = next(&Keymap::default(), Mode::Normal, &[Key::Char(' ')]);
        assert_eq!(next.len(), 14, "{next:?}");
        assert_eq!(next[0], (keys(&["f"]), "workspace::open_finder"));
        assert_eq!(next[1], (keys(&["/"]), "workspace::open_text_search"));
        assert_eq!(next[4], (keys(&["t"]), "panel::toggle_tags"));
    }

    #[test]
    fn las_teclas_de_una_misma_accion_van_juntas() {
        let next = next(&Keymap::default(), Mode::Normal, &[Key::Ctrl('w')]);
        assert_eq!(next[0], (keys(&["v", "ctrl-v"]), "pane::split_right"));
        assert_eq!(next.len(), 9, "{next:?}");
    }

    #[test]
    fn depende_del_modo() {
        let keymap = Keymap::default();
        let inner_word = Action::Vim(metalmind_vim::Action::TextObject(TextObject::Word {
            big: false,
            around: false,
        }));
        let pending = keymap.next_keys(Mode::OperatorPending(Operator::Delete), &[Key::Char('i')]);
        assert!(
            pending
                .iter()
                .any(|next| next.action == inner_word && next.keys == ["w"]),
            "{pending:?}"
        );
        assert_eq!(keymap.next_keys(Mode::Insert, &[Key::Char(' ')]), []);
        assert_eq!(keymap.next_keys(Mode::Normal, &[]), []);
    }

    #[test]
    fn no_sale_un_atajo_que_el_inicio_de_otro_tapa() {
        let mut bindings = Keymap::default().bindings().to_vec();
        let gz = vec![Key::Char('g'), Key::Char('Z'), Key::Char('Z')];
        bindings.extend([
            Binding {
                keys: Keys::Vim(VimContext::Normal, gz[..2].to_vec()),
                action: WindowCommand::OpenFinder.into(),
            },
            Binding {
                keys: Keys::Vim(VimContext::Normal, gz.clone()),
                action: WindowCommand::OpenTextSearch.into(),
            },
        ]);
        let keymap = Keymap::from_bindings(bindings);
        let next = next(&keymap, Mode::Normal, &[Key::Char('g')]);
        assert!(next.contains(&(keys(&["Z Z"]), "workspace::open_text_search")));
        assert!(
            next.iter()
                .all(|(_, name)| *name != "workspace::open_finder"),
            "{next:?}"
        );
    }

    #[test]
    fn de_dos_atajos_iguales_sale_el_que_gana() {
        let mut bindings = Keymap::default().bindings().to_vec();
        bindings.push(Binding {
            keys: Keys::Vim(VimContext::Normal, vec![Key::Char('g'), Key::Char('g')]),
            action: WindowCommand::OpenFinder.into(),
        });
        let keymap = Keymap::from_bindings(bindings);
        let next = next(&keymap, Mode::Normal, &[Key::Char('g')]);
        let gg: Vec<_> = next
            .iter()
            .filter(|(keys, _)| keys.contains(&"g".to_owned()))
            .collect();
        assert_eq!(gg, [&(keys(&["g"]), "workspace::open_finder")]);
        let start = Action::Vim(metalmind_vim::Action::Motion(Motion::DocumentStart));
        assert!(next.iter().all(|(_, name)| *name != start.name()));
    }
}
