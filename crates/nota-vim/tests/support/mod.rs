//! Notación de los tests: `|` marca cada cursor. En Normal va antes del
//! carácter bajo el cursor; en Insert, entre caracteres. En Visual, `¦`
//! marca el otro extremo de la selección si no coincide con el cursor.

pub mod nvim;

use nota_buffer::{Buffer, ByteOffset, Selection, Selections};
use nota_vim::{Key, KeyOutcome, Mode, Viewport, Vim, VisualKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub text: String,
    pub mode: String,
    /// Como `text`, pero en Visual también con el otro extremo (`¦`).
    pub selection: String,
}

const ANCHOR: char = '¦';

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

/// El cursor y, si hay selección Visual, su otro extremo.
pub fn render_visual(buffer: &Buffer, anchor: Option<ByteOffset>, head: ByteOffset) -> String {
    let mut text = buffer.to_string();
    let mut marks = vec![(head.get(), '|')];
    if let Some(anchor) = anchor.filter(|&anchor| anchor != head) {
        marks.push((anchor.get(), ANCHOR));
    }
    marks.sort_unstable();
    for (at, mark) in marks.into_iter().rev() {
        text.insert(at, mark);
    }
    text
}

/// Lo que llega al motor: una tecla, un texto pegado entero o el ratón.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    Key(Key),
    Paste(String),
    /// Clic que pone el cursor en esa línea y columna (en caracteres).
    Click {
        line: usize,
        column: usize,
    },
    /// Soltar el botón no le llega al motor; nvim lo necesita para que el
    /// clic no quede como arrastre.
    Release,
}

const PASTE_START: &str = "<PasteStart>";
const PASTE_END: &str = "<PasteEnd>";

/// Notación de vim (`<Esc>`, `<CR>`, `<BS>`, `<C-r>`), la misma que recibe
/// nvim. `<PasteStart>texto<PasteEnd>` es un pegado de la terminal y
/// `<LeftMouse><col,fila>` un clic, contando desde 0; en texto ASCII la
/// columna de pantalla de nvim es la de caracteres.
pub fn inputs(notation: &str) -> Vec<Input> {
    let mut inputs = Vec::new();
    let mut rest = notation;
    while let Some(c) = rest.chars().next() {
        if let Some(pasted) = rest.strip_prefix(PASTE_START) {
            let (text, after) = pasted.split_once(PASTE_END).expect("falta <PasteEnd>");
            inputs.push(Input::Paste(text.to_owned()));
            rest = after;
        } else if let Some((input, len)) = mouse(rest) {
            inputs.push(input);
            rest = &rest[len..];
        } else if let Some((key, len)) = special_key(rest) {
            inputs.push(Input::Key(key));
            rest = &rest[len..];
        } else {
            inputs.push(Input::Key(Key::Char(c)));
            rest = &rest[c.len_utf8()..];
        }
    }
    inputs
}

/// Pasa `notation` al motor y devuelve lo que pasó con la última entrada.
pub fn feed(
    vim: &mut Vim,
    buffer: &mut Buffer,
    selections: &mut Selections,
    notation: &str,
) -> Option<KeyOutcome> {
    let mut last = None;
    for input in inputs(notation) {
        let outcome = match input {
            Input::Key(key) => vim.feed(key, buffer, selections),
            Input::Paste(text) => vim.insert_text(&text, buffer, selections),
            Input::Click { line, column } => {
                assert_eq!(vim.mode(), Mode::Insert, "el clic solo se modela en Insert");
                let offset = buffer.offset_at_column(line, column);
                *selections = Selections::single(Selection::cursor(offset));
                vim.cursor_moved();
                continue;
            }
            Input::Release => continue,
        };
        last = Some(outcome.unwrap());
    }
    last
}

fn mouse(text: &str) -> Option<(Input, usize)> {
    let (button, rest) = text
        .strip_prefix("<LeftMouse>")
        .map(|rest| (true, rest))
        .or_else(|| text.strip_prefix("<LeftRelease>").map(|rest| (false, rest)))?;
    let end = rest.find('>')?;
    let (column, line) = rest.strip_prefix('<')?[..end - 1].split_once(',')?;
    let input = if button {
        Input::Click {
            line: line.parse().ok()?,
            column: column.parse().ok()?,
        }
    } else {
        Input::Release
    };
    Some((input, text.len() - rest.len() + end + 1))
}

fn special_key(text: &str) -> Option<(Key, usize)> {
    let end = text.strip_prefix('<')?.find('>')? + 1;
    let key = match &text[1..end] {
        "Esc" => Key::Escape,
        "CR" => Key::Enter,
        "BS" => Key::Backspace,
        "Tab" => Key::Tab,
        "Up" => Key::Up,
        "Down" => Key::Down,
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
        Mode::Visual(VisualKind::Char) => "v",
        Mode::Visual(VisualKind::Line) => "V",
        Mode::CommandLine => "c",
    }
}

/// Una ventana de tamaño fijo: cuántas filas caben, `scrolloff` y la
/// primera línea visible al empezar, desde 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Screen {
    pub rows: usize,
    pub scrolloff: usize,
    pub top: usize,
}

pub fn run_engine(before: &str, keys: &str) -> State {
    run_engine_on(before, keys, None).0
}

/// Como [`run_engine`], en una ventana como `screen`; devuelve además la
/// primera línea visible.
pub fn run_engine_on(before: &str, keys: &str, screen: Option<Screen>) -> (State, usize) {
    let (mut buffer, mut selections) = parse(before);
    let mut vim = Vim::new();
    if let Some(screen) = screen {
        let mut viewport = Viewport::new(screen.rows);
        let moved = viewport.scroll_to(
            screen.top,
            &buffer,
            &mut selections,
            screen.scrolloff,
            Mode::Normal,
        );
        assert!(!moved, "el cursor del caso tiene que verse en la ventana");
        viewport.reveal(&buffer, selections.primary().head, screen.scrolloff);
        vim.set_scrolloff(screen.scrolloff);
        vim.swap_viewport(&mut viewport);
    }
    feed(&mut vim, &mut buffer, &mut selections, keys);
    let visual = matches!(vim.mode(), Mode::Visual(_));
    let primary = selections.primary();
    let state = State {
        text: render(&buffer, selections.iter().map(|s| s.head)),
        mode: mode_name(vim.mode()).to_owned(),
        selection: render_visual(&buffer, visual.then_some(primary.anchor), primary.head),
    };
    let mut viewport = Viewport::default();
    vim.swap_viewport(&mut viewport);
    (state, viewport.top())
}
