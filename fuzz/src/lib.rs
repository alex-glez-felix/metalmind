//! Piezas comunes de los targets: teclas arbitrarias, cursores válidos y las
//! invariantes del motor vim.

use arbitrary::{Arbitrary, Unstructured};
use metalmind_buffer::{Buffer, ByteOffset, Selection, Selections};
use metalmind_vim::{Clipboard, Key, Mode, SystemRegister, Vim};

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

/// Comandos de varias teclas que el fuzzer casi nunca armaría tecla por
/// tecla: una macro solo se reproduce si antes se grabó en la misma letra.
#[derive(Clone, Debug, Arbitrary)]
pub enum Chord {
    /// `v`.
    Visual,
    /// `V`.
    VisualLine,
    /// `"{registro}`, antes de un operador o un pegado.
    Register(FuzzRegister),
    /// `q{registro}`: empieza a grabar.
    Record(FuzzRegister),
    /// `q`: termina la grabación.
    StopRecording,
    /// `@{registro}`.
    Play(FuzzRegister),
    /// `@@`.
    PlayLast,
    /// `m{mark}`.
    SetMark(FuzzMark),
    /// `'{mark}`, o `` `{mark} `` si `exact`.
    JumpToMark { mark: FuzzMark, exact: bool },
    /// `/{patrón}<CR>`, o `?` si `backward`, tecleado para que lo grabe `q`.
    Search {
        backward: bool,
        pattern: Vec<FuzzChar>,
    },
    /// `n`.
    Next,
    /// `N`.
    Previous,
}

/// Registros y nombres de mark, con algunos que no existen para que fallen.
const REGISTERS: &[char] = &[
    'a', 'b', 'z', 'A', 'B', '"', '0', '1', '2', '9', '-', '_', '.', '/', '+', '*', ':', '%', '=',
];
const MARKS: &[char] = &['a', 'b', 'z', '\'', '`', 'A', '0', '[', '<', '.'];

#[derive(Clone, Copy, Debug)]
pub struct FuzzRegister(pub char);

impl<'a> Arbitrary<'a> for FuzzRegister {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self(*u.choose(REGISTERS)?))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FuzzMark(pub char);

impl<'a> Arbitrary<'a> for FuzzMark {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self(*u.choose(MARKS)?))
    }
}

impl Chord {
    #[must_use]
    pub fn keys(&self) -> Vec<Key> {
        match self {
            Self::Visual => vec![Key::Char('v')],
            Self::VisualLine => vec![Key::Char('V')],
            Self::Register(register) => vec![Key::Char('"'), Key::Char(register.0)],
            Self::Record(register) => vec![Key::Char('q'), Key::Char(register.0)],
            Self::StopRecording => vec![Key::Char('q')],
            Self::Play(register) => vec![Key::Char('@'), Key::Char(register.0)],
            Self::PlayLast => vec![Key::Char('@'), Key::Char('@')],
            Self::SetMark(mark) => vec![Key::Char('m'), Key::Char(mark.0)],
            Self::JumpToMark { mark, exact } => {
                let jump = if *exact { '`' } else { '\'' };
                vec![Key::Char(jump), Key::Char(mark.0)]
            }
            Self::Search { backward, pattern } => {
                let prompt = if *backward { '?' } else { '/' };
                let typed = pattern.iter().map(|c| Key::Char(c.0));
                std::iter::once(Key::Char(prompt))
                    .chain(typed)
                    .chain([Key::Enter])
                    .collect()
            }
            Self::Next => vec![Key::Char('n')],
            Self::Previous => vec![Key::Char('N')],
        }
    }
}

/// Lo que le llega al motor desde quien lo aloja.
#[derive(Clone, Debug, Arbitrary)]
pub enum Input {
    Key(FuzzKey),
    Chord(Chord),
    /// Texto pegado entero, como `Document::paste` en Insert o en `:`.
    Paste(String),
    /// Clic que deja un solo cursor, como `Document::select`.
    Click(u16),
    /// Otra aplicación deja texto en `"+` o, si `primary`, en `"*`.
    CopyElsewhere {
        primary: bool,
        text: String,
    },
}

/// El portapapeles del sistema, que otra aplicación puede cambiar entre
/// tecla y tecla.
#[derive(Debug, Default)]
pub struct FuzzClipboard {
    clipboard: Option<String>,
    primary: Option<String>,
}

impl FuzzClipboard {
    fn slot(&mut self, register: SystemRegister) -> &mut Option<String> {
        match register {
            SystemRegister::Clipboard => &mut self.clipboard,
            SystemRegister::Primary => &mut self.primary,
        }
    }
}

impl Clipboard for FuzzClipboard {
    fn read(&mut self, register: SystemRegister) -> Option<String> {
        self.slot(register).clone()
    }

    fn write(&mut self, register: SystemRegister, text: String) {
        *self.slot(register) = Some(text);
    }
}

/// Corta los counts de más de dos dígitos: `999999999i` es tan lento en
/// Metalmind como en vim y no es un bug.
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

/// Quien aloja el motor: el texto, sus selecciones y el portapapeles.
pub struct Host {
    pub buffer: Buffer,
    pub selections: Selections,
    pub clipboard: FuzzClipboard,
}

impl Host {
    #[must_use]
    pub fn new(text: &str, cursors: &[u16]) -> Self {
        let buffer = Buffer::from(text);
        let selections = initial_selections(&buffer, cursors);
        Self {
            buffer,
            selections,
            clipboard: FuzzClipboard::default(),
        }
    }

    pub fn feed(&mut self, vim: &mut Vim, input: &Input) {
        match input {
            Input::Key(key) => self.press(vim, key.key()),
            Input::Chord(chord) => {
                for key in chord.keys() {
                    self.press(vim, key);
                }
            }
            Input::Paste(text) => {
                if let Err(error) = vim.paste_text(text, &mut self.buffer, &mut self.selections) {
                    panic!("pegar {text:?} dio un error de buffer: {error}");
                }
            }
            Input::Click(raw) => {
                vim.cursor_moved();
                let cursor = cursor_at(&self.buffer, *raw, vim.mode());
                self.selections = Selections::single(Selection::cursor(cursor));
            }
            Input::CopyElsewhere { primary, text } => {
                let register = if *primary {
                    SystemRegister::Primary
                } else {
                    SystemRegister::Clipboard
                };
                self.clipboard.write(register, text.clone());
            }
        }
    }

    pub fn press(&mut self, vim: &mut Vim, key: Key) {
        let result = vim.feed_with_clipboard(
            key,
            &mut self.buffer,
            &mut self.selections,
            &mut self.clipboard,
        );
        if let Err(error) = result {
            panic!("{key:?} dio un error de buffer, que el motor promete que no pasa: {error}");
        }
    }

    pub fn check(&self, vim: &Vim) {
        check_engine(vim, &self.buffer, &self.selections);
    }

    #[must_use]
    pub fn too_long(&self) -> bool {
        self.buffer.len().get() > MAX_BUFFER_LEN
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
