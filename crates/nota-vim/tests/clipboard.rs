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

use nota_buffer::{Buffer, Selections};
use nota_vim::{Clipboard, SystemRegister, Vim};
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
