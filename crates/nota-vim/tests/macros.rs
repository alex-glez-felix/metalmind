#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

//! Lo de las macros que nvim no puede mostrar en la tabla de `vim.rs`.

#[allow(
    dead_code,
    reason = "cada binario de test usa solo una parte del soporte compartido"
)]
mod support;

use nota_vim::{Clipboard, Command, KeyOutcome, SystemRegister, Vim};
use pretty_assertions::assert_eq;

/// Un sistema sin portapapeles: `"+` y `"*` siempre vacíos.
struct NoClipboard;

impl Clipboard for NoClipboard {
    fn read(&mut self, _: SystemRegister) -> Option<String> {
        None
    }

    fn write(&mut self, _: SystemRegister, _: String) {}
}

fn engine(marked: &str) -> (Vim, nota_buffer::Buffer, nota_buffer::Selections) {
    let (buffer, selections) = support::parse(marked);
    (Vim::new(), buffer, selections)
}

#[test]
fn reports_the_register_being_recorded() {
    let (mut vim, mut buffer, mut selections) = engine("|ab");
    assert_eq!(vim.recording(), None);
    support::feed(&mut vim, &mut buffer, &mut selections, "qBx");
    assert_eq!(vim.recording(), Some('b'));
    support::feed(&mut vim, &mut buffer, &mut selections, "q");
    assert_eq!(vim.recording(), None);
}

#[test]
fn a_macro_hands_its_commands_to_the_host() {
    let (mut vim, mut buffer, mut selections) = engine("|ab");
    let typed = support::feed(&mut vim, &mut buffer, &mut selections, "qax:w<CR>");
    assert_eq!(typed, Some(KeyOutcome::Command(Command::Save.into())));
    support::feed(&mut vim, &mut buffer, &mut selections, "q");
    let played = support::feed(&mut vim, &mut buffer, &mut selections, "@a");
    assert_eq!(played, Some(KeyOutcome::Command(Command::Save.into())));
    assert_eq!(buffer.to_string(), "");
}

#[test]
fn text_pasted_while_recording_is_part_of_the_macro() {
    let (mut vim, mut buffer, mut selections) = engine("|ab\ncd");
    let keys = "qaI<PasteStart>ñ!<PasteEnd><Esc>jq@a";
    support::feed(&mut vim, &mut buffer, &mut selections, keys);
    assert_eq!(buffer.to_string(), "ñ!ab\nñ!cd");
}

#[test]
fn a_macro_runs_on_every_cursor() {
    let state = support::run_engine("|abc |def", "qaxq@a");
    assert_eq!(state.text, "|c |f");
}

/// Vim la corta con `ctrl-c`; aquí no hay cómo, así que tiene un tope.
#[test]
fn a_macro_that_never_fails_stops_eventually() {
    let (mut vim, mut buffer, mut selections) = engine("|ab");
    support::feed(&mut vim, &mut buffer, &mut selections, "qaq");
    support::feed(&mut vim, &mut buffer, &mut selections, "qa0@aq@a");
    support::feed(&mut vim, &mut buffer, &mut selections, "x");
    assert_eq!(buffer.to_string(), "b");
}

/// Las repeticiones no se copian a la cola: un count enorme no reserva
/// memoria para todas antes de empezar.
#[test]
fn a_huge_count_does_not_queue_every_repetition() {
    let state = support::run_engine("|abc", "qalq999999999@ax");
    assert_eq!(state.text, "a|b");
}

#[test]
fn a_macro_pauses_on_a_host_command_until_resumed() {
    let (mut vim, mut buffer, mut selections) = engine("|abc");
    support::feed(&mut vim, &mut buffer, &mut selections, "qa<C-w>lxq");
    assert_eq!(buffer.to_string(), "bc");
    let played = support::feed(&mut vim, &mut buffer, &mut selections, "@a");
    assert_eq!(
        played,
        Some(KeyOutcome::Command(Command::FocusRight.into()))
    );
    assert_eq!(buffer.to_string(), "bc", "la x espera al otro panel");
    let (mut other, mut other_selections) = support::parse("|xyz");
    let resumed = vim
        .resume_macro(&mut other, &mut other_selections, &mut NoClipboard)
        .unwrap();
    assert_eq!(resumed, None);
    assert_eq!(other.to_string(), "yz");
    assert_eq!(
        vim.resume_macro(&mut buffer, &mut selections, &mut NoClipboard)
            .unwrap(),
        None
    );
}

#[test]
fn aborting_drops_the_rest_of_the_macro() {
    let (mut vim, mut buffer, mut selections) = engine("|abc");
    support::feed(&mut vim, &mut buffer, &mut selections, "qa<C-w>lxq@a");
    vim.abort_macro();
    let resumed = vim
        .resume_macro(&mut buffer, &mut selections, &mut NoClipboard)
        .unwrap();
    assert_eq!((resumed, buffer.to_string().as_str()), (None, "bc"));
}
