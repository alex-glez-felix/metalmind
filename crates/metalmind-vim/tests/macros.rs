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

use metalmind_keymap::Command;
use metalmind_vim::Vim;
use pretty_assertions::assert_eq;

#[test]
fn reports_the_register_being_recorded() {
    let mut editor = support::Editor::app("|ab");
    assert_eq!(editor.vim.recording(), None);
    editor.keys("qBx");
    assert_eq!(editor.vim.recording(), Some('b'));
    editor.keys("q");
    assert_eq!(editor.vim.recording(), None);
}

#[test]
fn a_macro_hands_its_commands_to_the_host() {
    let mut editor = support::Editor::app("|ab");
    let typed = editor.keys("qax:w<CR>");
    assert_eq!(typed, Some(support::dispatched(Command::Save)));
    editor.keys("q");
    let played = editor.keys("@a");
    assert_eq!(played, Some(support::dispatched(Command::Save)));
    assert_eq!(editor.text(), "");
}

#[test]
fn text_pasted_while_recording_is_part_of_the_macro() {
    let mut editor = support::Editor::app("|ab\ncd");
    let keys = "qaI<PasteStart>ñ!<PasteEnd><Esc>jq@a";
    editor.keys(keys);
    assert_eq!(editor.text(), "ñ!ab\nñ!cd");
}

#[test]
fn a_macro_runs_on_every_cursor() {
    let state = support::run_engine("|abc |def", "qaxq@a");
    assert_eq!(state.text, "|c |f");
}

/// Vim la corta con `ctrl-c`; aquí no hay cómo, así que tiene un tope.
#[test]
fn a_macro_that_never_fails_stops_eventually() {
    let mut editor = support::Editor::app("|ab");
    editor.keys("qaq");
    editor.keys("qa0@aq@a");
    editor.keys("x");
    assert_eq!(editor.text(), "b");
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
    let mut editor = support::Editor::app("|abc");
    editor.keys("qa<C-w>lxq");
    assert_eq!(editor.text(), "bc");
    let played = editor.keys("@a");
    assert_eq!(played, Some(support::dispatched(Command::FocusRight)));
    assert_eq!(editor.text(), "bc", "la x espera al otro panel");
    let mut other = support::Editor::new("|xyz");
    std::mem::swap(&mut editor.vim, &mut other.vim);
    let resumed = other.run(Vim::resume_macro).unwrap();
    assert_eq!(resumed, None);
    assert_eq!(other.text(), "yz");
    std::mem::swap(&mut editor.vim, &mut other.vim);
    assert_eq!(editor.run(Vim::resume_macro).unwrap(), None);
}

#[test]
fn aborting_drops_the_rest_of_the_macro() {
    let mut editor = support::Editor::app("|abc");
    editor.keys("qa<C-w>lxq@a");
    editor.vim.abort_macro();
    let resumed = editor.run(Vim::resume_macro).unwrap();
    assert_eq!((resumed, editor.text().as_str()), (None, "bc"));
}
