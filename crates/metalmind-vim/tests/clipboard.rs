#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

//! `"+` y `"*` con un portapapeles de prueba: el oráculo no los cubre porque
//! nvim usaría el portapapeles del sistema.

#[allow(
    dead_code,
    reason = "cada binario de test usa solo una parte del soporte compartido"
)]
mod support;

use metalmind_buffer::{Buffer, Selections};
use metalmind_vim::{Action, Clipboard, Mode, Operator, PasteSide, SystemRegister, Vim};
use pretty_assertions::assert_eq;

#[derive(Default)]
struct FakeClipboard {
    clipboard: Option<String>,
    primary: Option<String>,
    reads: usize,
}

impl Clipboard for FakeClipboard {
    fn read(&mut self, register: SystemRegister) -> Option<String> {
        self.reads += 1;
        match register {
            SystemRegister::Clipboard => self.clipboard.clone(),
            SystemRegister::Primary => self.primary.clone(),
        }
    }

    fn write(&mut self, register: SystemRegister, text: String) {
        match register {
            SystemRegister::Clipboard => self.clipboard = Some(text),
            SystemRegister::Primary => self.primary = Some(text),
        }
    }
}

struct Editor {
    vim: Vim,
    buffer: Buffer,
    selections: Selections,
    clipboard: FakeClipboard,
}

impl Editor {
    fn new(marked: &str) -> Self {
        let (buffer, selections) = support::parse(marked);
        Self {
            vim: Vim::new(),
            buffer,
            selections,
            clipboard: FakeClipboard::default(),
        }
    }

    fn keys(&mut self, notation: &str) -> &mut Self {
        for input in support::inputs(notation) {
            let support::Input::Key(key) = input else {
                panic!("solo teclas");
            };
            self.vim
                .feed_with_clipboard(
                    key,
                    &mut self.buffer,
                    &mut self.selections,
                    &mut self.clipboard,
                )
                .unwrap();
        }
        self
    }

    fn text(&self) -> String {
        support::render(&self.buffer, self.selections.iter().map(|s| s.head))
    }
}

#[test]
fn yanks_go_to_the_system_clipboard_with_lines_ending_in_newline() {
    let mut editor = Editor::new("|ab cd\nef");
    editor.keys("\"+yy\"*yw");
    assert_eq!(editor.clipboard.clipboard.as_deref(), Some("ab cd\n"));
    assert_eq!(editor.clipboard.primary.as_deref(), Some("ab "));
}

#[test]
fn deleting_into_the_clipboard_also_fills_the_numbered_registers() {
    let mut editor = Editor::new("|ab\ncd");
    editor.keys("\"+ddp\"1p");
    assert_eq!(editor.clipboard.clipboard.as_deref(), Some("ab\n"));
    assert_eq!(editor.text(), "cd\nab\n|ab");
}

#[test]
fn pasting_reads_lines_when_the_clipboard_ends_in_newline() {
    let mut editor = Editor::new("|ab\ncd");
    editor.clipboard.clipboard = Some("xy\n".to_owned());
    editor.clipboard.primary = Some("z".to_owned());
    editor.keys("\"+p\"*P");
    assert_eq!(editor.text(), "ab\n|zxy\ncd");
}

#[test]
fn an_empty_clipboard_pastes_nothing() {
    let mut editor = Editor::new("|ab");
    editor.clipboard.clipboard = Some(String::new());
    editor.keys("\"+p\"*p");
    assert_eq!(editor.text(), "|ab");
}

#[test]
fn only_the_clipboard_registers_read_the_clipboard() {
    let mut editor = Editor::new("|ab");
    editor.keys("ylp\"ap\"0p");
    assert_eq!(editor.clipboard.reads, 0);
    editor.keys("\"+p");
    assert_eq!(editor.clipboard.reads, 1);
}

#[test]
fn repeating_a_clipboard_paste_reads_it_again() {
    let mut editor = Editor::new("|ab");
    editor.clipboard.clipboard = Some("x".to_owned());
    editor.keys("\"+p");
    editor.clipboard.clipboard = Some("y".to_owned());
    editor.keys(".");
    assert_eq!(editor.text(), "ax|yb");
}

/// Sin portapapeles del sistema, como nvim sin proveedor: registros normales.
#[test]
fn without_a_clipboard_they_are_plain_registers() {
    let state = support::run_engine("|ab\ncd", "\"+yyj\"*yw\"+p\"*P");
    assert_eq!(state.text, "ab\ncd\nc|dab");
}

impl Editor {
    fn with_system_register(&mut self, action: Action) -> &mut Self {
        self.vim.use_register(SystemRegister::Clipboard);
        self.vim
            .execute_with_clipboard(
                action,
                &mut self.buffer,
                &mut self.selections,
                &mut self.clipboard,
            )
            .unwrap();
        self
    }
}

const YANK: Action = Action::Operator(Operator::Yank);
const PUT: Action = Action::Paste(PasteSide::Before);

#[test]
fn a_system_register_action_in_visual_is_like_typing_it() {
    let mut editor = Editor::new("|ab cd\nef");
    editor.keys("ve").with_system_register(YANK);
    assert_eq!(editor.clipboard.clipboard.as_deref(), Some("ab"));
    assert_eq!(editor.vim.mode(), Mode::Normal);
    editor.keys("jV").with_system_register(YANK);
    assert_eq!(editor.clipboard.clipboard.as_deref(), Some("ef\n"));
    editor.clipboard.clipboard = Some("X".to_owned());
    editor.keys("ggwve").with_system_register(PUT);
    assert_eq!(editor.text(), "ab |X\nef");
}

/// Lo pendiente no se completa con la acción: `r` no reemplaza por `"`, `3`
/// no pega tres veces, `"a` no se usa y `i` no se vuelve `i"`.
#[test]
fn a_system_register_action_drops_what_was_pending() {
    for pending in ["r", "3", "\"a", "i", "f"] {
        let mut editor = Editor::new("|ab cd\nef");
        editor.clipboard.clipboard = Some("X".to_owned());
        editor.keys("ve").keys(pending).with_system_register(PUT);
        assert_eq!(editor.text(), "|X cd\nef", "tras {pending:?}");
        assert_eq!(editor.vim.mode(), Mode::Normal, "tras {pending:?}");
    }
}

/// Como nvim con un proveedor, un bloque va al portapapeles con sus líneas,
/// sin `\n` final, y de vuelta se pega por caracteres.
#[test]
fn a_block_goes_to_the_clipboard_as_its_lines() {
    let mut editor = Editor::new("a|bc\ndef");
    editor.keys("<C-v>jl\"+y");
    assert_eq!(editor.clipboard.clipboard.as_deref(), Some("bc\nef"));
    editor.keys("\"+P");
    assert_eq!(editor.text(), "a|bc\nefbc\ndef");
}
