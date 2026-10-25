use std::collections::HashMap;

use crate::{
    Action, ActionName, CharSearch, HostCommand, InsertAt, Key, Mode, Motion, Operator, PasteSide,
    Place, Scroll, TextObject, VisualKind,
};

/// Dónde aplica un binding, al estilo de los contextos de Zed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Context {
    /// Normal, operador pendiente o Visual: donde valen las motions.
    VimControl,
    Normal,
    OperatorPending,
    /// Con este operador pendiente, p. ej. la segunda `d` de `dd`.
    Operator(Operator),
    /// Operador pendiente o Visual: donde valen los text objects.
    Object,
    Visual,
    Insert,
    CommandLine,
}

impl Context {
    #[must_use]
    pub fn matches(self, mode: Mode) -> bool {
        match (self, mode) {
            (Self::VimControl, Mode::Normal | Mode::OperatorPending(_) | Mode::Visual(_))
            | (Self::Normal, Mode::Normal)
            | (Self::OperatorPending, Mode::OperatorPending(_))
            | (Self::Object, Mode::OperatorPending(_) | Mode::Visual(_))
            | (Self::Visual, Mode::Visual(_))
            | (Self::Insert, Mode::Insert)
            | (Self::CommandLine, Mode::CommandLine) => true,
            (Self::Operator(wanted), Mode::OperatorPending(pending)) => wanted == pending,
            _ => false,
        }
    }
}

/// En `context`, la secuencia `keys` invoca `bound`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub context: Context,
    pub keys: Vec<Key>,
    pub bound: Bound,
}

/// Lo que invoca un binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bound {
    Engine(Action),
    /// Una acción ajena: el motor devuelve su nombre a quien lo aloja, como
    /// los paneles de `ctrl-w`, que así comparten prefijo, count y
    /// cancelación con el resto de Normal.
    Host(ActionName),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Lookup {
    Action(Bound),
    /// Las teclas son el inicio de algún binding.
    Pending,
    Unbound,
}

/// Bindings en orden; ante dos iguales gana el último, así una config de
/// usuario puede sobreescribir los defaults. Lleva también los comandos de
/// `:` ajenos al motor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keymap {
    bindings: Vec<Binding>,
    /// Los índices en `bindings` de los que empiezan con cada tecla, en
    /// orden: así cada tecla mira unos pocos y no todos.
    by_first_key: HashMap<Key, Vec<usize>>,
    commands: Vec<HostCommand>,
}

impl Keymap {
    #[must_use]
    pub fn new(
        bindings: impl IntoIterator<Item = Binding>,
        commands: impl IntoIterator<Item = HostCommand>,
    ) -> Self {
        let bindings: Vec<Binding> = bindings.into_iter().collect();
        let mut by_first_key: HashMap<Key, Vec<usize>> = HashMap::new();
        for (index, binding) in bindings.iter().enumerate() {
            if let Some(&first) = binding.keys.first() {
                by_first_key.entry(first).or_default().push(index);
            }
        }
        Self {
            bindings,
            by_first_key,
            commands: commands.into_iter().collect(),
        }
    }

    /// Las teclas de vim por defecto, solo las de edición: los atajos y los
    /// comandos de `:` ajenos los agrega quien aloja el editor.
    #[must_use]
    pub fn vim() -> Self {
        let bindings = VIM.iter().map(|&(context, keys, action)| Binding {
            context,
            keys: keys.to_vec(),
            bound: Bound::Engine(action),
        });
        Self::new(bindings, [])
    }

    #[must_use]
    pub fn bindings(&self) -> &[Binding] {
        &self.bindings
    }

    pub(crate) fn commands(&self) -> &[HostCommand] {
        &self.commands
    }

    pub(crate) fn lookup(&self, mode: Mode, keys: &[Key]) -> Lookup {
        let candidates = keys
            .first()
            .and_then(|first| self.by_first_key.get(first))
            .map_or(&[][..], Vec::as_slice);
        let mut exact = None;
        for binding in candidates.iter().rev().map(|&index| &self.bindings[index]) {
            if !binding.context.matches(mode) || !binding.keys.starts_with(keys) {
                continue;
            }
            if binding.keys.len() > keys.len() {
                return Lookup::Pending;
            }
            exact.get_or_insert(binding.bound);
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

const fn paragraph(around: bool) -> Action {
    object(TextObject::Paragraph { around })
}

const fn section(around: bool) -> Action {
    object(TextObject::Section { around })
}

const fn search_word(backward: bool, whole_word: bool) -> Action {
    Action::SearchWord {
        backward,
        whole_word,
    }
}

const fn reposition(place: Place, first_non_blank: bool) -> Action {
    Action::Reposition {
        place,
        first_non_blank,
    }
}

const fn operator(operator: Operator) -> Action {
    Action::Operator(operator)
}

const DELETE: Context = Context::Operator(Operator::Delete);
const CHANGE: Context = Context::Operator(Operator::Change);
const YANK: Context = Context::Operator(Operator::Yank);
const LOWERCASE: Context = Context::Operator(Operator::Lowercase);
const UPPERCASE: Context = Context::Operator(Operator::Uppercase);
const TOGGLE_CASE: Context = Context::Operator(Operator::ToggleCase);
const INDENT: Context = Context::Operator(Operator::Indent);
const OUTDENT: Context = Context::Operator(Operator::Outdent);

const VIM: &[(Context, &[Key], Action)] = {
    use Context::{CommandLine, Insert, Normal, Object, VimControl as Control, Visual};
    &[
        (Control, &[ch('h')], motion(Motion::Left)),
        (Control, &[ch('j')], motion(Motion::Down)),
        (Control, &[ch('k')], motion(Motion::Up)),
        (Control, &[Key::Ctrl('j')], motion(Motion::Down)),
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
        (Control, &[ch('%')], motion(Motion::Matching)),
        (Control, &[ch(']'), ch(']')], motion(Motion::NextHeading)),
        (
            Control,
            &[ch('['), ch('[')],
            motion(Motion::PreviousHeading),
        ),
        (Control, &[ch('f')], find(false, false)),
        (Control, &[ch('F')], find(true, false)),
        (Control, &[ch('t')], find(false, true)),
        (Control, &[ch('T')], find(true, true)),
        (Control, &[ch(';')], Action::RepeatFind { reversed: false }),
        (Control, &[ch(',')], Action::RepeatFind { reversed: true }),
        (Control, &[ch('\'')], Action::GoToMark { exact: false }),
        (Control, &[ch('`')], Action::GoToMark { exact: true }),
        (Normal, &[Key::Ctrl('o')], Action::JumpOlder),
        (Normal, &[Key::Ctrl('i')], Action::JumpNewer),
        (Normal, &[Key::Tab], Action::JumpNewer),
        (Control, &[Key::Escape], Action::NormalMode),
        (Normal, &[ch('d')], operator(Operator::Delete)),
        (Normal, &[ch('c')], operator(Operator::Change)),
        (Normal, &[ch('y')], operator(Operator::Yank)),
        (Normal, &[ch('g'), ch('u')], operator(Operator::Lowercase)),
        (Normal, &[ch('g'), ch('U')], operator(Operator::Uppercase)),
        (Normal, &[ch('g'), ch('~')], operator(Operator::ToggleCase)),
        (Normal, &[ch('>')], operator(Operator::Indent)),
        (Normal, &[ch('<')], operator(Operator::Outdent)),
        (Normal, &[ch('"')], Action::SelectRegister),
        (Normal, &[ch('x')], Action::DeleteRight),
        (Normal, &[ch('p')], Action::Paste(PasteSide::After)),
        (Normal, &[ch('P')], Action::Paste(PasteSide::Before)),
        (Normal, &[ch('.')], Action::RepeatChange),
        (Normal, &[ch('q')], Action::RecordMacro),
        (Normal, &[ch('@')], Action::PlayMacro),
        (Normal, &[ch('m')], Action::SetMark),
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
        (Normal, &[ch('v')], Action::Visual(VisualKind::Char)),
        (Normal, &[ch('V')], Action::Visual(VisualKind::Line)),
        (Normal, &[Key::Ctrl('v')], Action::Visual(VisualKind::Block)),
        (Normal, &[ch('g'), ch('v')], Action::RestoreVisual),
        (DELETE, &[ch('d')], Action::CurrentLine),
        (CHANGE, &[ch('c')], Action::CurrentLine),
        (YANK, &[ch('y')], Action::CurrentLine),
        (LOWERCASE, &[ch('u')], Action::CurrentLine),
        (UPPERCASE, &[ch('U')], Action::CurrentLine),
        (TOGGLE_CASE, &[ch('~')], Action::CurrentLine),
        (INDENT, &[ch('>')], Action::CurrentLine),
        (OUTDENT, &[ch('<')], Action::CurrentLine),
        (Visual, &[ch('v')], Action::Visual(VisualKind::Char)),
        (Visual, &[ch('V')], Action::Visual(VisualKind::Line)),
        (Visual, &[Key::Ctrl('v')], Action::Visual(VisualKind::Block)),
        (Visual, &[ch('o')], Action::OtherEnd),
        (Visual, &[ch('O')], Action::OtherCorner),
        (Visual, &[ch('I')], Action::BlockInsert { append: false }),
        (Visual, &[ch('A')], Action::BlockInsert { append: true }),
        (Visual, &[ch('g'), ch('v')], Action::RestoreVisual),
        (Visual, &[Key::Ctrl('c')], Action::Interrupt),
        (Visual, &[ch('d')], operator(Operator::Delete)),
        (Visual, &[ch('x')], operator(Operator::Delete)),
        (Visual, &[ch('c')], operator(Operator::Change)),
        (Visual, &[ch('y')], operator(Operator::Yank)),
        (Visual, &[ch('u')], operator(Operator::Lowercase)),
        (Visual, &[ch('U')], operator(Operator::Uppercase)),
        (Visual, &[ch('~')], operator(Operator::ToggleCase)),
        (Visual, &[ch('>')], operator(Operator::Indent)),
        (Visual, &[ch('<')], operator(Operator::Outdent)),
        (Visual, &[ch('p')], Action::Paste(PasteSide::After)),
        (Visual, &[ch('P')], Action::Paste(PasteSide::Before)),
        (Visual, &[ch('r')], Action::Replace),
        (Visual, &[ch('"')], Action::SelectRegister),
        (Visual, &[ch(':')], Action::CommandLine),
        (Object, &[ch('i'), ch('w')], word(false, false)),
        (Object, &[ch('a'), ch('w')], word(false, true)),
        (Object, &[ch('i'), ch('W')], word(true, false)),
        (Object, &[ch('a'), ch('W')], word(true, true)),
        (Object, &[ch('i'), ch('(')], parentheses(false)),
        (Object, &[ch('i'), ch(')')], parentheses(false)),
        (Object, &[ch('i'), ch('b')], parentheses(false)),
        (Object, &[ch('a'), ch('(')], parentheses(true)),
        (Object, &[ch('a'), ch(')')], parentheses(true)),
        (Object, &[ch('a'), ch('b')], parentheses(true)),
        (Object, &[ch('i'), ch('"')], quotes(false)),
        (Object, &[ch('a'), ch('"')], quotes(true)),
        (Object, &[ch('i'), ch('p')], paragraph(false)),
        (Object, &[ch('a'), ch('p')], paragraph(true)),
        (Object, &[ch('i'), ch('h')], section(false)),
        (Object, &[ch('a'), ch('h')], section(true)),
        (Insert, &[Key::Escape], Action::NormalMode),
        (Insert, &[Key::Ctrl('c')], Action::Interrupt),
        (Insert, &[Key::Backspace], Action::Backspace),
        (Insert, &[Key::Enter], Action::Newline),
        (Insert, &[Key::Ctrl('j')], Action::Newline),
        (CommandLine, &[Key::Escape], Action::NormalMode),
        (CommandLine, &[Key::Ctrl('c')], Action::Interrupt),
        (CommandLine, &[Key::Backspace], Action::Backspace),
        (CommandLine, &[Key::Enter], Action::SubmitCommand),
        (CommandLine, &[Key::Ctrl('j')], Action::SubmitCommand),
        (CommandLine, &[Key::Up], Action::HistoryPrevious),
        (CommandLine, &[Key::Down], Action::HistoryNext),
        // Búsqueda: `/`, `?` y lo que repite o busca la palabra del cursor.
        (Control, &[ch('/')], Action::Search { backward: false }),
        (Control, &[ch('?')], Action::Search { backward: true }),
        (Control, &[ch('n')], Action::SearchNext { reversed: false }),
        (Control, &[ch('N')], Action::SearchNext { reversed: true }),
        (Control, &[ch('*')], search_word(false, true)),
        (Control, &[ch('#')], search_word(true, true)),
        (Control, &[ch('g'), ch('*')], search_word(false, false)),
        (Control, &[ch('g'), ch('#')], search_word(true, false)),
        // Scroll: con un operador pendiente lo cancelan, como en vim.
        (Control, &[ch('H')], motion(Motion::Window(Place::Top))),
        (Control, &[ch('M')], motion(Motion::Window(Place::Middle))),
        (Control, &[ch('L')], motion(Motion::Window(Place::Bottom))),
        (Control, &[Key::Ctrl('e')], Action::Scroll(Scroll::LineDown)),
        (Control, &[Key::Ctrl('y')], Action::Scroll(Scroll::LineUp)),
        (
            Control,
            &[Key::Ctrl('d')],
            Action::Scroll(Scroll::HalfPageDown),
        ),
        (
            Control,
            &[Key::Ctrl('u')],
            Action::Scroll(Scroll::HalfPageUp),
        ),
        (Control, &[Key::Ctrl('f')], Action::Scroll(Scroll::PageDown)),
        (Control, &[Key::Ctrl('b')], Action::Scroll(Scroll::PageUp)),
        (Control, &[ch('z'), ch('t')], reposition(Place::Top, false)),
        (
            Control,
            &[ch('z'), ch('z')],
            reposition(Place::Middle, false),
        ),
        (
            Control,
            &[ch('z'), ch('b')],
            reposition(Place::Bottom, false),
        ),
        (
            Control,
            &[ch('z'), Key::Enter],
            reposition(Place::Top, true),
        ),
        (
            Control,
            &[ch('z'), ch('.')],
            reposition(Place::Middle, true),
        ),
        (
            Control,
            &[ch('z'), ch('-')],
            reposition(Place::Bottom, true),
        ),
    ]
};

#[cfg(test)]
mod tests {
    use super::*;

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

    fn overlap(a: Context, b: Context) -> bool {
        MODES.iter().any(|&mode| a.matches(mode) && b.matches(mode))
    }

    /// Una secuencia que fuera exacta y a la vez prefijo de otra necesitaría
    /// el timeout de vim para decidir; el default no debe tener ninguna.
    #[test]
    fn default_keymap_is_unambiguous() {
        let bindings = Keymap::vim().bindings;
        for (i, binding) in bindings.iter().enumerate() {
            let (context, keys, action) = (binding.context, &binding.keys, binding.bound);
            for other in &bindings[i + 1..] {
                let (other_context, other_keys, other_action) =
                    (other.context, &other.keys, other.bound);
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
            Lookup::Action(Bound::Engine(motion(Motion::DocumentStart)))
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
            Lookup::Action(Bound::Engine(Action::CurrentLine))
        );
        assert_eq!(
            keymap.lookup(pending(Operator::Yank), &[ch('d')]),
            Lookup::Unbound
        );
    }

    #[test]
    fn later_bindings_win() {
        let binding = |bound| Binding {
            context: Context::Normal,
            keys: vec![ch('x')],
            bound,
        };
        let open = Bound::Host(ActionName::new("workspace::open_finder"));
        let keymap = Keymap::new(
            [binding(Bound::Engine(Action::DeleteRight)), binding(open)],
            [],
        );
        assert_eq!(
            keymap.lookup(Mode::Normal, &[ch('x')]),
            Lookup::Action(open)
        );
    }
}
