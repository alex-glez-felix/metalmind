//! Patrones y reemplazos arbitrarios para `/`, `?` y `:s`: la traducción del
//! dialecto de vim a `regex` nunca entra en pánico y un patrón inválido da
//! error.

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use metalmind_buffer::{Buffer, ByteOffset, Selection, Selections};
use metalmind_fuzz::{FuzzChar, MAX_BUFFER_LEN, check_engine};
use metalmind_vim::{Key, KeyOutcome, Mode, Vim};

#[derive(Debug, Arbitrary)]
enum Op {
    Search {
        backward: bool,
        pattern: String,
    },
    /// `:{range}s{delim}{pattern}{delim}{replacement}{delim}{flags}`.
    Substitute {
        whole_file: bool,
        delimiter: FuzzChar,
        pattern: String,
        replacement: String,
        flags: String,
    },
    /// Lo escrito tras `:` tal cual, para lo que no arma `Substitute`.
    Ex(String),
    /// `n`, `N`, `*`, `#`, `&`, `u`, `ctrl-r`.
    Normal(u8),
}

const NORMAL: &[Key] = &[
    Key::Char('n'),
    Key::Char('N'),
    Key::Char('*'),
    Key::Char('#'),
    Key::Char('&'),
    Key::Char('u'),
    Key::Ctrl('r'),
];

#[derive(Debug, Arbitrary)]
struct Case {
    text: String,
    ops: Vec<Op>,
}

fn press(vim: &mut Vim, key: Key, buffer: &mut Buffer, selections: &mut Selections) -> KeyOutcome {
    match vim.feed(key, buffer, selections) {
        Ok(outcome) => outcome,
        Err(error) => panic!("{key:?} dio un error de buffer: {error}"),
    }
}

/// Escribe `line` en la línea de comandos tras `prompt` y la envía.
fn submit(
    vim: &mut Vim,
    prompt: char,
    line: &str,
    buffer: &mut Buffer,
    selections: &mut Selections,
) -> KeyOutcome {
    press(vim, Key::Char(prompt), buffer, selections);
    assert_eq!(vim.mode(), Mode::CommandLine, "{prompt} no abrió la línea");
    if let Err(error) = vim.insert_text(line, buffer, selections) {
        panic!("escribir {line:?} dio un error de buffer: {error}");
    }
    assert_eq!(vim.command_line(), Some(line));
    let outcome = press(vim, Key::Enter, buffer, selections);
    assert_eq!(vim.mode(), Mode::Normal, "{line:?} no cerró la línea");
    outcome
}

fn substitute_line(
    whole_file: bool,
    delimiter: char,
    pattern: &str,
    replacement: &str,
    flags: &str,
) -> String {
    let range = if whole_file { "%" } else { "" };
    format!("{range}s{delimiter}{pattern}{delimiter}{replacement}{delimiter}{flags}")
}

fuzz_target!(|case: Case| {
    let mut buffer = Buffer::from(case.text.as_str());
    let mut selections = Selections::single(Selection::cursor(ByteOffset::ZERO));
    let mut vim = Vim::new();
    for op in &case.ops {
        match op {
            Op::Search { backward, pattern } => {
                let prompt = if *backward { '?' } else { '/' };
                submit(&mut vim, prompt, pattern, &mut buffer, &mut selections);
            }
            Op::Substitute {
                whole_file,
                delimiter,
                pattern,
                replacement,
                flags,
            } => {
                let line = substitute_line(*whole_file, delimiter.0, pattern, replacement, flags);
                submit(&mut vim, ':', &line, &mut buffer, &mut selections);
            }
            Op::Ex(line) => {
                submit(&mut vim, ':', line, &mut buffer, &mut selections);
            }
            Op::Normal(index) => {
                let key = NORMAL[usize::from(*index) % NORMAL.len()];
                press(&mut vim, key, &mut buffer, &mut selections);
            }
        }
        check_engine(&vim, &buffer, &selections);
        if buffer.len().get() > MAX_BUFFER_LEN {
            return;
        }
    }
});
