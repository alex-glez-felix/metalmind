//! El panel de las teclas que siguen a un prefijo del editor, como `space`
//! (#86): qué muestra y tras cuánto, según `which_key` de `config.yaml`.

use std::time::Duration;

use metalmind_config::{Toggle, WhichKey};
use metalmind_keymap::{Keymap, NextKey};

use super::{Effect, Workspace};

/// Lo tecleado, como se muestra (`space`, `ctrl-w`), y lo que puede seguir.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NextKeys {
    pub typed: String,
    pub next: Vec<NextKey>,
}

impl Workspace {
    pub fn set_which_key(&mut self, which_key: WhichKey) {
        self.which_key = which_key;
    }

    /// Cuánto esperar con un prefijo tecleado antes de mostrar el panel;
    /// `None` si está apagado.
    #[must_use]
    pub fn which_key_delay(&self) -> Option<Duration> {
        self.which_key
            .enabled
            .then(|| self.which_key.delay_ms.get())
    }

    /// Lo que muestra el panel ahora: `None` si está apagado o si lo
    /// tecleado no es el inicio de ningún atajo de `keymap`.
    #[must_use]
    pub fn next_keys(&self, keymap: &Keymap) -> Option<NextKeys> {
        if !self.which_key.enabled {
            return None;
        }
        let typed = self.vim.pending_keys();
        let next = keymap.next_keys(self.vim.mode(), typed);
        if next.is_empty() {
            return None;
        }
        let typed = typed
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" ");
        Some(NextKeys { typed, next })
    }

    pub(super) fn toggle_which_key(&mut self) -> Effect {
        self.which_key.enabled = !self.which_key.enabled;
        Effect::SaveToggle {
            toggle: Toggle::WhichKey,
            on: self.which_key.enabled,
        }
    }
}

#[cfg(test)]
mod tests {
    use metalmind_keymap::{Command, Keymap};
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::workspace::test_session::Session;

    fn toggle(session: &mut Session) -> Effect {
        let toggle = Command::ToggleWhichKey.into();
        session.workspace.run(&toggle, &mut session.clipboard)
    }

    fn shown(session: &Session) -> Option<(String, Vec<&'static str>)> {
        let next_keys = session.workspace.next_keys(&Keymap::default())?;
        let actions = next_keys
            .next
            .iter()
            .map(|next| next.action.name())
            .collect();
        Some((next_keys.typed, actions))
    }

    #[test]
    fn con_un_prefijo_muestra_lo_que_sigue_y_se_va_con_la_siguiente_tecla() {
        let mut session = Session::new("|uno\ndos");
        assert_eq!(shown(&session), None);
        session.keys(" ");
        let (typed, actions) = shown(&session).unwrap();
        assert_eq!(typed, "space");
        assert_eq!(actions[0], "workspace::open_finder");
        session.keys("⎋");
        assert_eq!(shown(&session), None);
        session.keys("⌃w");
        assert_eq!(shown(&session).unwrap().0, "ctrl-w");
        session.keys("z");
        assert_eq!(shown(&session), None);
    }

    /// `d` cambia de modo sin dejar teclas a medias: el panel no muestra
    /// todas las motions.
    #[test]
    fn un_operador_no_es_un_prefijo() {
        let mut session = Session::new("|uno");
        session.keys("d");
        assert_eq!(shown(&session), None);
        session.keys("i");
        assert_eq!(shown(&session).unwrap().0, "i");
    }

    #[test]
    fn apagado_no_muestra_nada_y_el_comando_lo_enciende() {
        let mut session = Session::new("|uno");
        assert_eq!(
            session.workspace.which_key_delay(),
            Some(Duration::from_millis(500))
        );
        let effect = toggle(&mut session);
        assert_eq!(
            effect,
            Effect::SaveToggle {
                toggle: Toggle::WhichKey,
                on: false
            }
        );
        assert_eq!(session.workspace.which_key_delay(), None);
        session.keys(" ");
        assert_eq!(shown(&session), None);
        session.keys("⎋");
        assert_eq!(
            toggle(&mut session),
            Effect::SaveToggle {
                toggle: Toggle::WhichKey,
                on: true
            }
        );
        session.keys(" ");
        assert_eq!(shown(&session).unwrap().0, "space");
    }
}
