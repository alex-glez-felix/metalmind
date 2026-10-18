use crate::{Action, CharSearch, InsertAt, Key, Mode, Motion, Operator, PasteSide, TextObject};

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
    CommandLine,
}

impl Context {
    #[must_use]
    pub fn matches(self, mode: Mode) -> bool {
        match (self, mode) {
            (Self::VimControl, Mode::Normal | Mode::OperatorPending(_))
            | (Self::Normal, Mode::Normal)
            | (Self::OperatorPending, Mode::OperatorPending(_))
            | (Self::Insert, Mode::Insert)
            | (Self::CommandLine, Mode::CommandLine) => true,
            (Self::Operator(wanted), Mode::OperatorPending(pending)) => wanted == pending,
            _ => false,
        }
    }
}

/// En `context`, la secuencia `keys` invoca `action`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub context: Context,
    pub keys: Vec<Key>,
    pub action: Action,
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
    #[must_use]
    pub fn new(bindings: impl IntoIterator<Item = Binding>) -> Self {
        Self {
            bindings: bindings.into_iter().collect(),
        }
    }

    /// El keymap de vim por defecto.
    #[must_use]
    pub fn vim() -> Self {
        Self::new(VIM.iter().map(|&(context, keys, action)| Binding {
            context,
            keys: keys.to_vec(),
            action,
        }))
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

const fn ch(c: char) -> Key {
    Key::Char(c)
}

const fn motion(motion: Motion) -> Action {
    Action::Motion(motion)
}

const fn find(backward: bool, till: bool) -> Action {
    Action::FindChar(CharSearch { backward, till })
}

const fn object(object: TextObject) -> Action {
    Action::TextObject(object)
}

const fn word(big: bool, around: bool) -> Action {
    object(TextObject::Word { big, around })
}

const fn parentheses(around: bool) -> Action {
    object(TextObject::Parentheses { around })
}

const fn quotes(around: bool) -> Action {
    object(TextObject::DoubleQuotes { around })
}

const DELETE: Context = Context::Operator(Operator::Delete);
const CHANGE: Context = Context::Operator(Operator::Change);
const YANK: Context = Context::Operator(Operator::Yank);

const VIM: &[(Context, &[Key], Action)] = {
    use Context::{CommandLine, Insert, Normal, OperatorPending as Pending, VimControl as Control};
    &[
        (Control, &[ch('h')], motion(Motion::Left)),
        (Control, &[ch('j')], motion(Motion::Down)),
        (Control, &[ch('k')], motion(Motion::Up)),
        (Control, &[ch('l')], motion(Motion::Right)),
        (
            Control,
            &[ch('w')],
            motion(Motion::NextWordStart { big: false }),
        ),
        (
            Control,
            &[ch('W')],
            motion(Motion::NextWordStart { big: true }),
        ),
        (
            Control,
            &[ch('e')],
            motion(Motion::NextWordEnd { big: false }),
        ),
        (
            Control,
            &[ch('E')],
            motion(Motion::NextWordEnd { big: true }),
        ),
        (
            Control,
            &[ch('b')],
            motion(Motion::PreviousWordStart { big: false }),
        ),
        (
            Control,
            &[ch('B')],
            motion(Motion::PreviousWordStart { big: true }),
        ),
        (Control, &[ch('0')], motion(Motion::LineStart)),
        (Control, &[ch('^')], motion(Motion::FirstNonBlank)),
        (Control, &[ch('$')], motion(Motion::LineEnd)),
        (Control, &[ch('g'), ch('g')], motion(Motion::DocumentStart)),
        (Control, &[ch('G')], motion(Motion::DocumentEnd)),
        (Control, &[ch('f')], find(false, false)),
        (Control, &[ch('F')], find(true, false)),
        (Control, &[ch('t')], find(false, true)),
        (Control, &[ch('T')], find(true, true)),
        (Control, &[ch(';')], Action::RepeatFind { reversed: false }),
        (Control, &[ch(',')], Action::RepeatFind { reversed: true }),
        (Control, &[Key::Escape], Action::NormalMode),
        (Normal, &[ch('d')], Action::Operator(Operator::Delete)),
        (Normal, &[ch('c')], Action::Operator(Operator::Change)),
        (Normal, &[ch('y')], Action::Operator(Operator::Yank)),
        (Normal, &[ch('x')], Action::DeleteRight),
        (Normal, &[ch('p')], Action::Paste(PasteSide::After)),
        (Normal, &[ch('P')], Action::Paste(PasteSide::Before)),
        (Normal, &[ch('u')], Action::Undo),
        (Normal, &[Key::Ctrl('r')], Action::Redo),
        (Normal, &[ch('g'), ch('-')], Action::Earlier),
        (Normal, &[ch('g'), ch('+')], Action::Later),
        (Normal, &[ch('i')], Action::Insert(InsertAt::Before)),
        (Normal, &[ch('a')], Action::Insert(InsertAt::After)),
        (Normal, &[ch('I')], Action::Insert(InsertAt::FirstNonBlank)),
        (Normal, &[ch('A')], Action::Insert(InsertAt::LineEnd)),
        (Normal, &[ch('o')], Action::Insert(InsertAt::LineBelow)),
        (Normal, &[ch('O')], Action::Insert(InsertAt::LineAbove)),
        (Normal, &[ch(':')], Action::CommandLine),
        (DELETE, &[ch('d')], Action::CurrentLine),
        (CHANGE, &[ch('c')], Action::CurrentLine),
        (YANK, &[ch('y')], Action::CurrentLine),
        (Pending, &[ch('i'), ch('w')], word(false, false)),
        (Pending, &[ch('a'), ch('w')], word(false, true)),
        (Pending, &[ch('i'), ch('W')], word(true, false)),
        (Pending, &[ch('a'), ch('W')], word(true, true)),
        (Pending, &[ch('i'), ch('(')], parentheses(false)),
        (Pending, &[ch('i'), ch(')')], parentheses(false)),
        (Pending, &[ch('i'), ch('b')], parentheses(false)),
        (Pending, &[ch('a'), ch('(')], parentheses(true)),
        (Pending, &[ch('a'), ch(')')], parentheses(true)),
        (Pending, &[ch('a'), ch('b')], parentheses(true)),
        (Pending, &[ch('i'), ch('"')], quotes(false)),
        (Pending, &[ch('a'), ch('"')], quotes(true)),
        (Insert, &[Key::Escape], Action::NormalMode),
        (Insert, &[Key::Ctrl('c')], Action::Interrupt),
        (Insert, &[Key::Backspace], Action::Backspace),
        (Insert, &[Key::Enter], Action::Newline),
        (CommandLine, &[Key::Escape], Action::NormalMode),
        (CommandLine, &[Key::Ctrl('c')], Action::Interrupt),
        (CommandLine, &[Key::Backspace], Action::Backspace),
        (CommandLine, &[Key::Enter], Action::SubmitCommand),
    ]
};

#[cfg(test)]
mod tests {
    use super::*;

    const MODES: [Mode; 6] = [
        Mode::Normal,
        Mode::Insert,
        Mode::CommandLine,
        Mode::OperatorPending(Operator::Delete),
        Mode::OperatorPending(Operator::Change),
        Mode::OperatorPending(Operator::Yank),
    ];

    fn overlap(a: Context, b: Context) -> bool {
        MODES.iter().any(|&mode| a.matches(mode) && b.matches(mode))
    }

    /// Una secuencia que fuera exacta y a la vez prefijo de otra necesitaría
    /// el timeout de vim para decidir; el default no debe tener ninguna.
    #[test]
    fn default_keymap_is_unambiguous() {
        for (i, &(context, keys, action)) in VIM.iter().enumerate() {
            for &(other_context, other_keys, other_action) in &VIM[i + 1..] {
                if !overlap(context, other_context) {
                    continue;
                }
                let shorter = keys.len().min(other_keys.len());
                assert!(
                    keys[..shorter] != other_keys[..shorter] || keys.len() == other_keys.len(),
                    "{keys:?} y {other_keys:?} se pisan en {context:?}/{other_context:?}"
                );
                assert!(
                    keys != other_keys,
                    "{keys:?} está dos veces: {action:?} y {other_action:?}"
                );
            }
        }
    }

    #[test]
    fn multi_key_sequences_wait_for_the_rest() {
        let keymap = Keymap::vim();
        assert_eq!(keymap.lookup(Mode::Normal, &[ch('g')]), Lookup::Pending);
        assert_eq!(
            keymap.lookup(Mode::Normal, &[ch('g'), ch('g')]),
            Lookup::Action(motion(Motion::DocumentStart))
        );
        assert_eq!(
            keymap.lookup(Mode::Normal, &[ch('g'), ch('x')]),
            Lookup::Unbound
        );
    }

    #[test]
    fn operator_context_only_matches_its_operator() {
        let keymap = Keymap::vim();
        let pending = Mode::OperatorPending;
        assert_eq!(
            keymap.lookup(pending(Operator::Delete), &[ch('d')]),
            Lookup::Action(Action::CurrentLine)
        );
        assert_eq!(
            keymap.lookup(pending(Operator::Yank), &[ch('d')]),
            Lookup::Unbound
        );
    }

    #[test]
    fn later_bindings_win() {
        let binding = |action| Binding {
            context: Context::Normal,
            keys: vec![ch('x')],
            action,
        };
        let keymap = Keymap::new([binding(Action::DeleteRight), binding(Action::Undo)]);
        assert_eq!(
            keymap.lookup(Mode::Normal, &[ch('x')]),
            Lookup::Action(Action::Undo)
        );
    }
}
