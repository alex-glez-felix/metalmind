//! Los atajos de la ventana que se teclean en el editor, como `space f` o
//! `ctrl-w v`. Los de edición son los de vim ([`metalmind_vim::Keymap::vim`]).

use metalmind_vim::{Context as VimContext, Key};

use crate::{Binding, Command};

pub(crate) fn bindings() -> Vec<Binding> {
    let mut bindings = Vec::new();
    bindings.extend(leader());
    bindings.extend(panes());
    bindings.extend(links());
    bindings
}

fn in_normal(keys: Vec<Key>, command: Command) -> Binding {
    Binding {
        context: VimContext::Normal,
        keys,
        action: command.into(),
    }
}

/// `space` y una tecla, en Normal (03 §3).
const LEADER: &[(char, Command)] = &[
    ('f', Command::OpenFinder),
    ('/', Command::OpenTextSearch),
    ('p', Command::OpenCommandPalette),
    ('e', Command::ToggleFolders),
    ('t', Command::ToggleTags),
];

fn leader() -> impl Iterator<Item = Binding> {
    LEADER
        .iter()
        .map(|&(key, command)| in_normal(vec![Key::Char(' '), Key::Char(key)], command))
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

fn panes() -> impl Iterator<Item = Binding> {
    PANES.iter().flat_map(|&(key, command)| {
        [Key::Char(key), Key::Ctrl(key)].map(|next| in_normal(vec![Key::Ctrl('w'), next], command))
    })
}

fn links() -> impl Iterator<Item = Binding> {
    [vec![Key::Char('g'), Key::Char('d')], vec![Key::Enter]]
        .into_iter()
        .map(|keys| in_normal(keys, Command::FollowLink))
}
