//! Piezas comunes de los targets: teclas arbitrarias, cursores válidos y las
//! invariantes del motor vim.

use arbitrary::{Arbitrary, Unstructured};
use nota_buffer::{Buffer, ByteOffset, Selection, Selections};
use nota_vim::{Key, Mode, Vim};

/// Más que esto y una sola tecla con count tarda segundos: el fuzzer
/// buscaría lentitud esperada en vez de bugs.
pub const MAX_BUFFER_LEN: usize = 1 << 16;

/// Caracteres que significan algo para vim o que cuestan en UTF-8.
const INTERESTING: &[char] = &[
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's',
    't', 'u', 'v', 'w', 'x', 'y', 'z', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L',
    'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', '0', '1', '2', '3', '9',
    ' ', '\t', '\n', '.', ',', ';', ':', '/', '?', '\\', '"', '\'', '`', '@', '&', '*', '#', '$',
    '^', '%', '~', '!', '|', '-', '+', '=', '_', '<', '>', '(', ')', '[', ']', '{', '}', 'é', 'ñ',
    '中', '😀', '\u{a0}', '\u{301}',
];

const CTRL: &[char] = &['c', 'j', 'r', 'w', 'u', 'h', 'v', 'o', 'a', 'x', '['];

#[derive(Clone, Copy, Debug)]
pub struct FuzzChar(pub char);

impl<'a> Arbitrary<'a> for FuzzChar {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let index = u.choose_index(INTERESTING.len() + 1)?;
        Ok(Self(match INTERESTING.get(index) {
            Some(&c) => c,
            None => u.arbitrary()?,
        }))
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub enum FuzzKey {
    Char(FuzzChar),
    Ctrl(u8),
    Escape,
    Enter,
    Backspace,
    Tab,
    Up,
    Down,
}

impl FuzzKey {
    #[must_use]
    pub fn key(self) -> Key {
        match self {
            Self::Char(c) => Key::Char(c.0),
            Self::Ctrl(i) => Key::Ctrl(CTRL[usize::from(i) % CTRL.len()]),
            Self::Escape => Key::Escape,
            Self::Enter => Key::Enter,
            Self::Backspace => Key::Backspace,
            Self::Tab => Key::Tab,
            Self::Up => Key::Up,
            Self::Down => Key::Down,
        }
    }
}

/// Lo que le llega al motor desde quien lo aloja.
#[derive(Clone, Debug, Arbitrary)]
pub enum Input {
    Key(FuzzKey),
    /// Texto pegado entero, como `Document::paste` en Insert o en `:`.
    Paste(String),
    /// Clic que deja un solo cursor, como `Document::select`.
    Click(u16),
}

/// Corta los counts de más de dos dígitos: `999999999i` es tan lento en
/// nota como en vim y no es un bug.
#[derive(Default)]
pub struct CountLimit {
    digits: usize,
}

impl CountLimit {
    pub fn allows(&mut self, key: Key) -> bool {
        if matches!(key, Key::Char(c) if c.is_ascii_digit()) {
            self.digits += 1;
            self.digits <= 2
        } else {
            self.digits = 0;
            true
        }
    }
}

/// Lleva `raw` a una posición donde el host puede dejar un cursor en `mode`.
#[must_use]
pub fn cursor_at(buffer: &Buffer, raw: u16, mode: Mode) -> ByteOffset {
    let mut offset = ByteOffset::new(usize::from(raw) % (buffer.len().get() + 1));
    while !buffer.is_char_boundary(offset) {
        offset = offset.saturating_sub(1);
    }
    if mode == Mode::Insert {
        return offset;
    }
    let line = buffer.line_of(offset);
    if offset >= buffer.line_end(line) && offset > buffer.line_start(line) {
        buffer.prev_boundary(buffer.line_end(line))
    } else {
        offset
    }
}

/// Uno o varios cursores de Normal; nunca vacío.
#[must_use]
pub fn initial_selections(buffer: &Buffer, raw: &[u16]) -> Selections {
    let cursors = raw
        .iter()
        .take(4)
        .map(|&raw| Selection::cursor(cursor_at(buffer, raw, Mode::Normal)));
    Selections::new(cursors)
        .unwrap_or_else(|| Selections::single(Selection::cursor(ByteOffset::ZERO)))
}

pub fn feed(vim: &mut Vim, input: &Input, buffer: &mut Buffer, selections: &mut Selections) {
    let result = match input {
        Input::Key(key) => vim.feed(key.key(), buffer, selections),
        Input::Paste(text) => vim.insert_text(text, buffer, selections),
        Input::Click(raw) => {
            vim.cursor_moved();
            let cursor = cursor_at(buffer, *raw, vim.mode());
            *selections = Selections::single(Selection::cursor(cursor));
            return;
        }
    };
    if let Err(error) = result {
        panic!("{input:?} dio un error de buffer, que el motor promete que no pasa: {error}");
    }
}

/// Lo que debe cumplirse tras cada tecla, sea cual sea.
pub fn check_engine(vim: &Vim, buffer: &Buffer, selections: &Selections) {
    for selection in selections {
        for offset in [selection.anchor, selection.head] {
            assert!(
                buffer.is_char_boundary(offset),
                "{selection:?} cae fuera del texto o parte un carácter (largo {})",
                buffer.len()
            );
        }
    }
    let mode = vim.mode();
    assert_eq!(
        vim.command_line().is_some(),
        mode == Mode::CommandLine,
        "la línea de comandos no corresponde al modo {mode:?}"
    );
    match mode {
        Mode::Insert => assert!(
            buffer.has_open_transaction(),
            "en Insert lo tecleado va a una transacción abierta"
        ),
        // `/` desde Visual conserva la selección mientras se escribe.
        Mode::Visual(_) | Mode::CommandLine => assert!(
            !buffer.has_open_transaction(),
            "{mode:?} con una transacción abierta"
        ),
        Mode::Normal | Mode::OperatorPending(_) => {
            assert!(
                !buffer.has_open_transaction(),
                "{mode:?} con una transacción abierta"
            );
            for selection in selections {
                check_normal_cursor(buffer, selection, mode);
            }
        }
    }
}

/// En Normal hay solo cursores, y nunca sobre el fin de una línea con texto.
fn check_normal_cursor(buffer: &Buffer, selection: &Selection, mode: Mode) {
    assert!(
        selection.is_cursor(),
        "{selection:?} no es un cursor en {mode:?}"
    );
    let head = selection.head;
    let line = buffer.line_of(head);
    let on_line_end = head == buffer.line_end(line) && head > buffer.line_start(line);
    assert!(
        !on_line_end,
        "en {mode:?} el cursor {head} quedó sobre el fin de la línea {line}"
    );
}
