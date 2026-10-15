//! Notación de los tests: `|` marca cada cursor. En Normal va antes del
//! carácter bajo el cursor; en Insert, entre caracteres.

pub mod nvim;

use nota_buffer::{Buffer, ByteOffset, Selection, Selections};
use nota_vim::{Key, Mode, Vim};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub text: String,
    pub mode: String,
}

pub fn parse(marked: &str) -> (Buffer, Selections) {
    let mut text = String::new();
    let mut cursors = Vec::new();
    for c in marked.chars() {
        if c == '|' {
            cursors.push(Selection::cursor(ByteOffset::new(text.len())));
        } else {
            text.push(c);
        }
    }
    let selections = Selections::new(cursors).expect("el caso necesita al menos un `|`");
    (Buffer::from(text.as_str()), selections)
}

pub fn render(buffer: &Buffer, heads: impl IntoIterator<Item = ByteOffset>) -> String {
    let mut text = buffer.to_string();
    let mut heads: Vec<usize> = heads.into_iter().map(ByteOffset::get).collect();
    heads.sort_unstable();
    for head in heads.into_iter().rev() {
        text.insert(head, '|');
    }
    text
}

/// Notación de vim (`<Esc>`, `<CR>`, `<BS>`, `<C-r>`), la misma que recibe nvim.
pub fn vim_keys(notation: &str) -> Vec<Key> {
    let mut keys = Vec::new();
    let mut rest = notation;
    while let Some(c) = rest.chars().next() {
        if let Some((key, len)) = special_key(rest) {
            keys.push(key);
            rest = &rest[len..];
        } else {
            keys.push(Key::Char(c));
            rest = &rest[c.len_utf8()..];
        }
    }
    keys
}

fn special_key(text: &str) -> Option<(Key, usize)> {
    let end = text.strip_prefix('<')?.find('>')? + 1;
    let key = match &text[1..end] {
        "Esc" => Key::Escape,
        "CR" => Key::Enter,
        "BS" => Key::Backspace,
        "Tab" => Key::Tab,
        "lt" => Key::Char('<'),
        name => Key::Ctrl(name.strip_prefix("C-")?.chars().next()?),
    };
    Some((key, end + 1))
}

pub fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Normal => "n",
        Mode::Insert => "i",
        Mode::OperatorPending(_) => "no",
    }
}

pub fn run_engine(before: &str, keys: &str) -> State {
    let (mut buffer, mut selections) = parse(before);
    let mut vim = Vim::new();
    for key in vim_keys(keys) {
        vim.feed(key, &mut buffer, &mut selections).unwrap();
    }
    State {
        text: render(&buffer, selections.iter().map(|s| s.head)),
        mode: mode_name(vim.mode()).to_owned(),
    }
}
