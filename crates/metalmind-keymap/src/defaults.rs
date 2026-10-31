//! Los atajos de la ventana por defecto. Los de edición son los de vim
//! ([`metalmind_vim::Keymap::vim`]); aquí va todo lo demás, también lo que
//! se teclea en el editor pero no es de vim, como `space f` o `ctrl-w v`.

use metalmind_vim::{Context as VimContext, Key};

use crate::{Action, Binding, Command, EditorAction, KeyName, Keys, Keystroke, ListAction};
use crate::{ChangePreviewAction, PanelAction, PickerAction, View};

pub(crate) fn bindings() -> Vec<Binding> {
    // `q` pliega la lista, como `space l`; va antes para que la lista muestre
    // `space l`, el último que se declara.
    let mut bindings = vec![
        in_view(View::NoteList, &[k('q')], Command::ToggleNoteList),
        in_view(View::NoteList, &[k('s')], Command::SaveListView),
    ];
    bindings.extend(leader());
    bindings.extend(panes());
    bindings.extend(links());
    bindings.extend(editor());
    bindings.extend(table(View::Completion, COMPLETION));
    bindings.extend(table(View::CommandCompletion, COMMAND_COMPLETION));
    bindings.extend(table(View::FileTree, TREE));
    bindings.extend(table(View::FileTreeInput, TREE_INPUT));
    bindings.extend(table(View::FileTreeConfirm, TREE_CONFIRM));
    bindings.extend(table(View::FileTreeMenu, TREE_MENU));
    bindings.extend(table(View::Picker, PICKER));
    bindings.extend(table(View::PickerWithSplits, PICKER_SPLITS));
    bindings.extend(table(View::ChangePreview, CHANGE_PREVIEW));
    bindings.extend(table(View::NoteList, LIST));
    bindings.extend(table(View::NoteListInput, LIST_INPUT));
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

/// `space` y una tecla, en Normal (03 §3). Con `also_in_panels`, la misma
/// tecla vale en el árbol del panel lateral y en la lista de notas.
const LEADER: &[(char, Command, bool)] = &[
    ('f', Command::OpenFinder, false),
    ('/', Command::OpenTextSearch, false),
    ('p', Command::OpenCommandPalette, false),
    ('e', Command::ToggleFolders, true),
    ('t', Command::ToggleTags, true),
    ('b', Command::ToggleBacklinks, true),
    ('u', Command::OpenUnresolvedLinks, true),
    ('l', Command::ToggleNoteList, true),
    ('v', Command::ChooseListSource, true),
    ('c', Command::ShowFolder, false),
    ('n', Command::NewNoteFromTemplate, true),
    ('d', Command::OpenDailyNote, true),
];

fn leader() -> impl Iterator<Item = Binding> {
    LEADER.iter().flat_map(|&(key, command, also_in_panels)| {
        let editor = in_normal(vec![Key::Char(' '), Key::Char(key)], command);
        let panels = [View::FileTree, View::NoteList]
            .into_iter()
            .filter(move |_| also_in_panels)
            .map(move |view| in_view(view, &[k(' '), k(key)], command));
        std::iter::once(editor).chain(panels)
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

/// Desde el árbol y la lista, que están a la izquierda de los paneles.
const PANES_FROM_TREE: &[(char, PanelAction)] = &[('l', PanelAction::FocusEditor)];
const PANES_FROM_LIST: &[(char, ListAction)] = &[
    ('l', ListAction::FocusEditor),
    ('h', ListAction::FocusSidebar),
];

fn panes() -> impl Iterator<Item = Binding> {
    let editor = PANES.iter().flat_map(|&(key, command)| {
        [Key::Char(key), Key::Ctrl(key)].map(|next| in_normal(vec![Key::Ctrl('w'), next], command))
    });
    let tree = PANES_FROM_TREE.iter().flat_map(|&(key, action)| {
        [k(key), ctrl(key)].map(|next| in_view(View::FileTree, &[ctrl('w'), next], action))
    });
    let list = PANES_FROM_LIST.iter().flat_map(|&(key, action)| {
        [k(key), ctrl(key)].map(|next| in_view(View::NoteList, &[ctrl('w'), next], action))
    });
    editor.chain(tree).chain(list)
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

/// Como en los pickers, menos `enter`, que en Insert sigue siendo un salto
/// de línea (05 §3).
/// Solo `tab`: las flechas, `ctrl-n` y `ctrl-p` son del historial.
const COMMAND_COMPLETION: &[(&[Keystroke], EditorAction)] =
    &[(&[named(KeyName::Tab)], EditorAction::AcceptCompletion)];

const COMPLETION: &[(&[Keystroke], EditorAction)] = &[
    (&[named(KeyName::Tab)], EditorAction::AcceptCompletion),
    (&[ctrl('n')], EditorAction::NextCompletion),
    (&[named(KeyName::Down)], EditorAction::NextCompletion),
    (&[ctrl('p')], EditorAction::PreviousCompletion),
    (&[named(KeyName::Up)], EditorAction::PreviousCompletion),
];

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
    (&[k('e')], PanelAction::EditTagAliases),
    (&[k('p')], PanelAction::EditTagParents),
    (&[k('i')], PanelAction::SetTagIcon),
    (&[k('t')], PanelAction::SetTagTemplate),
    (&[k('P')], PanelAction::OpenTagPage),
    (&[k('F')], PanelAction::ToggleFocus),
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

/// Como el árbol; `/` y `space f` filtran la lista en su lugar con la
/// consulta del finder, y `t` etiqueta la nota para vaciar el buzón. De dos
/// teclas para lo mismo, la lista muestra la última.
const LIST: &[(&[Keystroke], ListAction)] = &[
    (&[k('j')], ListAction::SelectNext),
    (&[named(KeyName::Down)], ListAction::SelectNext),
    (&[k('k')], ListAction::SelectPrevious),
    (&[named(KeyName::Up)], ListAction::SelectPrevious),
    (&[k('g'), k('g')], ListAction::SelectFirst),
    (&[k('G')], ListAction::SelectLast),
    (&[k('o')], ListAction::Open),
    (&[named(KeyName::Enter)], ListAction::Open),
    (&[ctrl('v')], ListAction::OpenSplitRight),
    (&[ctrl('s')], ListAction::OpenSplitDown),
    (&[k('/')], ListAction::Filter),
    (&[k(' '), k('f')], ListAction::Filter),
    (&[k('t')], ListAction::Tag),
    (&[named(KeyName::Escape)], ListAction::Cancel),
];

/// Mientras se escribe, las flechas y `ctrl-n`/`ctrl-p` siguen moviendo la
/// fila, como en un picker.
const LIST_INPUT: &[(&[Keystroke], ListAction)] = &[
    (&[named(KeyName::Enter)], ListAction::Confirm),
    (&[named(KeyName::Escape)], ListAction::Cancel),
    (&[named(KeyName::Down)], ListAction::SelectNext),
    (&[ctrl('n')], ListAction::SelectNext),
    (&[named(KeyName::Up)], ListAction::SelectPrevious),
    (&[ctrl('p')], ListAction::SelectPrevious),
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

/// Como el árbol: `j`/`k` se mueven, `x` quita un cambio como `x` borra un
/// carácter, `enter` aplica y `esc` o `q` cierran sin tocar nada.
const CHANGE_PREVIEW: &[(&[Keystroke], ChangePreviewAction)] = &[
    (&[k('j')], ChangePreviewAction::SelectNext),
    (&[named(KeyName::Down)], ChangePreviewAction::SelectNext),
    (&[ctrl('n')], ChangePreviewAction::SelectNext),
    (&[k('k')], ChangePreviewAction::SelectPrevious),
    (&[named(KeyName::Up)], ChangePreviewAction::SelectPrevious),
    (&[ctrl('p')], ChangePreviewAction::SelectPrevious),
    (&[k('x')], ChangePreviewAction::ToggleChange),
    (&[named(KeyName::Enter)], ChangePreviewAction::Apply),
    (&[k('q')], ChangePreviewAction::Cancel),
    (&[named(KeyName::Escape)], ChangePreviewAction::Cancel),
];
