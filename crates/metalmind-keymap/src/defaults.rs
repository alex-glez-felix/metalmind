//! Los atajos de la ventana por defecto. Los de edición son los de vim
//! ([`metalmind_vim::Keymap::vim`]); aquí va todo lo demás, también lo que
//! se teclea en el editor pero no es de vim, como `space f` o `ctrl-w v`.

use metalmind_vim::{Context as VimContext, Key};

use crate::{Action, Binding, Command, EditorAction, KeyName, Keys, Keystroke, PanelAction};
use crate::{PickerAction, View};

pub(crate) fn bindings() -> Vec<Binding> {
    let mut bindings = Vec::new();
    bindings.extend(leader());
    bindings.extend(panes());
    bindings.extend(links());
    bindings.extend(editor());
    bindings.extend(table(View::FileTree, TREE));
    bindings.extend(table(View::FileTreeInput, TREE_INPUT));
    bindings.extend(table(View::FileTreeConfirm, TREE_CONFIRM));
    bindings.extend(table(View::FileTreeMenu, TREE_MENU));
    bindings.extend(table(View::Picker, PICKER));
    bindings.extend(table(View::PickerWithSplits, PICKER_SPLITS));
    bindings
}

const fn k(c: char) -> Keystroke {
    Keystroke::plain(KeyName::Char(c))
}

const fn named(key: KeyName) -> Keystroke {
    Keystroke::plain(key)
}

const fn ctrl(c: char) -> Keystroke {
    Keystroke::ctrl(KeyName::Char(c))
}

fn in_normal(keys: Vec<Key>, action: impl Into<Action>) -> Binding {
    Binding {
        keys: Keys::Vim(VimContext::Normal, keys),
        action: action.into(),
    }
}

fn in_view(view: View, keys: &[Keystroke], action: impl Into<Action>) -> Binding {
    Binding {
        keys: Keys::View(view, keys.to_vec()),
        action: action.into(),
    }
}

fn table<A: Into<Action> + Copy>(
    view: View,
    rows: &'static [(&'static [Keystroke], A)],
) -> impl Iterator<Item = Binding> {
    rows.iter()
        .map(move |&(keys, action)| in_view(view, keys, action))
}

/// `space` y una tecla, en Normal (03 §3). Con `also_in_tree`, la misma
/// tecla vale en el árbol del panel lateral.
const LEADER: &[(char, Command, bool)] = &[
    ('f', Command::OpenFinder, false),
    ('/', Command::OpenTextSearch, false),
    ('p', Command::OpenCommandPalette, false),
    ('e', Command::ToggleFolders, true),
    ('t', Command::ToggleTags, true),
];

fn leader() -> impl Iterator<Item = Binding> {
    LEADER.iter().flat_map(|&(key, command, also_in_tree)| {
        let editor = in_normal(vec![Key::Char(' '), Key::Char(key)], command);
        let tree = also_in_tree.then(|| in_view(View::FileTree, &[k(' '), k(key)], command));
        std::iter::once(editor).chain(tree)
    })
}

/// `ctrl-w` seguido de la tecla, con o sin ctrl, como acepta vim. Van en
/// el motor porque `ctrl-w` comparte prefijo, count y cancelación con el
/// resto de Normal.
const PANES: &[(char, Command)] = &[
    ('v', Command::SplitRight),
    ('s', Command::SplitDown),
    ('h', Command::FocusLeft),
    ('j', Command::FocusDown),
    ('k', Command::FocusUp),
    ('l', Command::FocusRight),
    ('w', Command::FocusNext),
    ('q', Command::Quit),
    ('o', Command::Only),
];

/// Desde el árbol, que está a la izquierda de los paneles.
const PANES_FROM_TREE: &[(char, PanelAction)] = &[('l', PanelAction::FocusEditor)];

fn panes() -> impl Iterator<Item = Binding> {
    let editor = PANES.iter().flat_map(|&(key, command)| {
        [Key::Char(key), Key::Ctrl(key)].map(|next| in_normal(vec![Key::Ctrl('w'), next], command))
    });
    let tree = PANES_FROM_TREE.iter().flat_map(|&(key, action)| {
        [k(key), ctrl(key)].map(|next| in_view(View::FileTree, &[ctrl('w'), next], action))
    });
    editor.chain(tree)
}

fn links() -> impl Iterator<Item = Binding> {
    [vec![Key::Char('g'), Key::Char('d')], vec![Key::Enter]]
        .into_iter()
        .map(|keys| in_normal(keys, Command::FollowLink))
}

/// `ctrl-shift-*` no lo usa vim, así que queda para el portapapeles del
/// sistema y para la paleta, con el atajo de Zed.
fn editor() -> impl Iterator<Item = Binding> {
    let shortcut = |c| [Keystroke::ctrl_shift(KeyName::Char(c))];
    [
        in_view(View::Editor, &shortcut('c'), EditorAction::Copy),
        in_view(View::Editor, &shortcut('v'), EditorAction::Paste),
        in_view(View::Editor, &shortcut('p'), Command::OpenCommandPalette),
    ]
    .into_iter()
}

const TREE: &[(&[Keystroke], PanelAction)] = &[
    (&[k('j')], PanelAction::SelectNext),
    (&[named(KeyName::Down)], PanelAction::SelectNext),
    (&[k('k')], PanelAction::SelectPrevious),
    (&[named(KeyName::Up)], PanelAction::SelectPrevious),
    (&[k('g'), k('g')], PanelAction::SelectFirst),
    (&[k('G')], PanelAction::SelectLast),
    (&[k('h')], PanelAction::CollapseOrParent),
    (&[named(KeyName::Left)], PanelAction::CollapseOrParent),
    (&[k('l')], PanelAction::ExpandOrOpen),
    (&[named(KeyName::Right)], PanelAction::ExpandOrOpen),
    (&[k('o')], PanelAction::Open),
    (&[named(KeyName::Enter)], PanelAction::Open),
    (&[ctrl('v')], PanelAction::OpenSplitRight),
    (&[ctrl('s')], PanelAction::OpenSplitDown),
    (&[k('/')], PanelAction::Filter),
    (&[k('a')], PanelAction::NewNote),
    (&[k('A')], PanelAction::NewFolder),
    (&[k('r')], PanelAction::Rename),
    (&[k('m')], PanelAction::Move),
    (&[k('d')], PanelAction::Delete),
    (&[k('T')], PanelAction::ToggleView),
    (&[k('q')], PanelAction::Hide),
    (&[named(KeyName::Escape)], PanelAction::Cancel),
];

const TREE_INPUT: &[(&[Keystroke], PanelAction)] = &[
    (&[named(KeyName::Enter)], PanelAction::Confirm),
    (&[named(KeyName::Escape)], PanelAction::Cancel),
];

const TREE_CONFIRM: &[(&[Keystroke], PanelAction)] = &[
    (&[k('y')], PanelAction::Confirm),
    (&[named(KeyName::Enter)], PanelAction::Confirm),
    (&[k('n')], PanelAction::Cancel),
    (&[named(KeyName::Escape)], PanelAction::Cancel),
];

const TREE_MENU: &[(&[Keystroke], PanelAction)] = &[
    (&[k('j')], PanelAction::MenuNext),
    (&[named(KeyName::Down)], PanelAction::MenuNext),
    (&[k('k')], PanelAction::MenuPrevious),
    (&[named(KeyName::Up)], PanelAction::MenuPrevious),
    (&[named(KeyName::Enter)], PanelAction::Confirm),
    (&[named(KeyName::Escape)], PanelAction::Cancel),
];

const PICKER: &[(&[Keystroke], PickerAction)] = &[
    (&[named(KeyName::Down)], PickerAction::SelectNext),
    (&[ctrl('n')], PickerAction::SelectNext),
    (&[ctrl('j')], PickerAction::SelectNext),
    (&[named(KeyName::Up)], PickerAction::SelectPrevious),
    (&[ctrl('p')], PickerAction::SelectPrevious),
    (&[ctrl('k')], PickerAction::SelectPrevious),
    (&[named(KeyName::Enter)], PickerAction::Confirm),
    (&[named(KeyName::Escape)], PickerAction::Dismiss),
    (&[ctrl('c')], PickerAction::Dismiss),
    (&[named(KeyName::Backspace)], PickerAction::DeleteBackward),
    (&[named(KeyName::Delete)], PickerAction::DeleteForward),
    (&[ctrl('w')], PickerAction::DeleteWordBackward),
    (
        &[Keystroke::ctrl(KeyName::Backspace)],
        PickerAction::DeleteWordBackward,
    ),
    (&[ctrl('u')], PickerAction::DeleteToStart),
    (&[named(KeyName::Left)], PickerAction::MoveLeft),
    (&[named(KeyName::Right)], PickerAction::MoveRight),
    (&[named(KeyName::Home)], PickerAction::MoveToStart),
    (&[ctrl('a')], PickerAction::MoveToStart),
    (&[named(KeyName::End)], PickerAction::MoveToEnd),
    (&[ctrl('e')], PickerAction::MoveToEnd),
];

const PICKER_SPLITS: &[(&[Keystroke], PickerAction)] = &[
    (&[ctrl('v')], PickerAction::ConfirmSplitRight),
    (&[ctrl('s')], PickerAction::ConfirmSplitDown),
];
