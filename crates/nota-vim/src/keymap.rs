use crate::{Action, Key, KeymapError, Mode, Operator};

/// Dónde aplica un binding, al estilo de los contextos de Zed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Context {
    /// Normal u operador pendiente: donde valen las motions.
    VimControl,
    Normal,
    OperatorPending,
    /// Con este operador pendiente, p. ej. la segunda `d` de `dd`.
    Operator(Operator),
    Insert,
}

impl Context {
    #[must_use]
    pub fn matches(self, mode: Mode) -> bool {
        match (self, mode) {
            (Self::VimControl, Mode::Normal | Mode::OperatorPending(_))
            | (Self::Normal, Mode::Normal)
            | (Self::OperatorPending, Mode::OperatorPending(_))
            | (Self::Insert, Mode::Insert) => true,
            (Self::Operator(wanted), Mode::OperatorPending(pending)) => wanted == pending,
            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Binding {
    context: Context,
    keys: Vec<Key>,
    action: Action,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Lookup {
    Action(Action),
    /// Las teclas son el inicio de algún binding.
    Pending,
    Unbound,
}

/// Bindings en orden; ante dos iguales gana el último, así una config de
/// usuario puede sobreescribir los defaults.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keymap {
    bindings: Vec<Binding>,
}

impl Keymap {
    /// # Errors
    /// Si alguna secuencia de teclas o nombre de acción no existe.
    pub fn new<'a>(
        bindings: impl IntoIterator<Item = (Context, &'a str, &'a str)>,
    ) -> Result<Self, KeymapError> {
        let bindings = bindings
            .into_iter()
            .map(|(context, keys, action)| {
                Ok(Binding {
                    context,
                    keys: Key::parse_sequence(keys)?,
                    action: Action::from_name(action)
                        .ok_or_else(|| KeymapError::UnknownAction(action.to_owned()))?,
                })
            })
            .collect::<Result<_, KeymapError>>()?;
        Ok(Self { bindings })
    }

    /// # Errors
    /// Solo si el keymap embebido está mal escrito; lo cubre un test.
    pub fn vim() -> Result<Self, KeymapError> {
        Self::new(VIM.iter().copied())
    }

    pub(crate) fn lookup(&self, mode: Mode, keys: &[Key]) -> Lookup {
        let mut exact = None;
        for binding in self.bindings.iter().rev() {
            if !binding.context.matches(mode) || !binding.keys.starts_with(keys) {
                continue;
            }
            if binding.keys.len() > keys.len() {
                return Lookup::Pending;
            }
            exact.get_or_insert(binding.action);
        }
        exact.map_or(Lookup::Unbound, Lookup::Action)
    }
}

const VIM: &[(Context, &str, &str)] = &[
    (Context::VimControl, "h", "vim::left"),
    (Context::VimControl, "j", "vim::down"),
    (Context::VimControl, "k", "vim::up"),
    (Context::VimControl, "l", "vim::right"),
    (Context::VimControl, "w", "vim::next_word_start"),
    (Context::VimControl, "W", "vim::next_big_word_start"),
    (Context::VimControl, "e", "vim::next_word_end"),
    (Context::VimControl, "E", "vim::next_big_word_end"),
    (Context::VimControl, "b", "vim::previous_word_start"),
    (Context::VimControl, "B", "vim::previous_big_word_start"),
    (Context::VimControl, "0", "vim::start_of_line"),
    (Context::VimControl, "^", "vim::first_non_blank"),
    (Context::VimControl, "$", "vim::end_of_line"),
    (Context::VimControl, "g g", "vim::start_of_document"),
    (Context::VimControl, "G", "vim::end_of_document"),
    (Context::VimControl, "escape", "vim::normal_mode"),
    (Context::Normal, "d", "vim::delete"),
    (Context::Normal, "c", "vim::change"),
    (Context::Normal, "y", "vim::yank"),
    (Context::Normal, "x", "vim::delete_right"),
    (Context::Normal, "p", "vim::paste_after"),
    (Context::Normal, "P", "vim::paste_before"),
    (Context::Normal, "u", "vim::undo"),
    (Context::Normal, "ctrl-r", "vim::redo"),
    (Context::Normal, "i", "vim::insert_before"),
    (Context::Normal, "a", "vim::insert_after"),
    (Context::Normal, "I", "vim::insert_first_non_blank"),
    (Context::Normal, "A", "vim::insert_end_of_line"),
    (Context::Normal, "o", "vim::insert_line_below"),
    (Context::Normal, "O", "vim::insert_line_above"),
    (
        Context::Operator(Operator::Delete),
        "d",
        "vim::current_line",
    ),
    (
        Context::Operator(Operator::Change),
        "c",
        "vim::current_line",
    ),
    (Context::Operator(Operator::Yank), "y", "vim::current_line"),
    (Context::OperatorPending, "i w", "vim::inner_word"),
    (Context::OperatorPending, "a w", "vim::a_word"),
    (Context::OperatorPending, "i W", "vim::inner_big_word"),
    (Context::OperatorPending, "a W", "vim::a_big_word"),
    (Context::OperatorPending, "i (", "vim::inner_parentheses"),
    (Context::OperatorPending, "i )", "vim::inner_parentheses"),
    (Context::OperatorPending, "i b", "vim::inner_parentheses"),
    (Context::OperatorPending, "a (", "vim::a_parentheses"),
    (Context::OperatorPending, "a )", "vim::a_parentheses"),
    (Context::OperatorPending, "a b", "vim::a_parentheses"),
    (Context::OperatorPending, "i \"", "vim::inner_double_quotes"),
    (Context::OperatorPending, "a \"", "vim::a_double_quotes"),
    (Context::Insert, "escape", "vim::normal_mode"),
    (Context::Insert, "backspace", "editor::backspace"),
    (Context::Insert, "enter", "editor::newline"),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(sequence: &str) -> Vec<Key> {
        Key::parse_sequence(sequence).unwrap()
    }

    #[test]
    fn default_keymap_parses() {
        assert!(Keymap::vim().is_ok());
    }

    #[test]
    fn multi_key_sequences_wait_for_the_rest() {
        let keymap = Keymap::vim().unwrap();
        assert_eq!(keymap.lookup(Mode::Normal, &keys("g")), Lookup::Pending);
        assert_eq!(
            keymap.lookup(Mode::Normal, &keys("g g")),
            Lookup::Action(Action::Motion(crate::Motion::DocumentStart))
        );
        assert_eq!(keymap.lookup(Mode::Normal, &keys("g x")), Lookup::Unbound);
    }

    #[test]
    fn operator_context_only_matches_its_operator() {
        let keymap = Keymap::vim().unwrap();
        let pending = |op| Mode::OperatorPending(op);
        assert_eq!(
            keymap.lookup(pending(Operator::Delete), &keys("d")),
            Lookup::Action(Action::CurrentLine)
        );
        assert_eq!(
            keymap.lookup(pending(Operator::Yank), &keys("d")),
            Lookup::Unbound
        );
    }

    #[test]
    fn later_bindings_win() {
        let keymap = Keymap::new([
            (Context::Normal, "x", "vim::delete_right"),
            (Context::Normal, "x", "vim::undo"),
        ])
        .unwrap();
        assert_eq!(
            keymap.lookup(Mode::Normal, &keys("x")),
            Lookup::Action(Action::Undo)
        );
    }

    #[test]
    fn unknown_names_are_errors() {
        assert_eq!(
            Keymap::new([(Context::Normal, "x", "vim::nope")]),
            Err(KeymapError::UnknownAction("vim::nope".into()))
        );
    }
}
