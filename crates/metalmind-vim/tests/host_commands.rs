#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

//! Los comandos de `:` que ejecuta quien aloja el editor: que cada forma que
//! acepta Metalmind sea el mismo comando para nvim.

#[allow(
    dead_code,
    reason = "cada binario de test usa solo una parte del soporte compartido"
)]
mod support;

use metalmind_keymap::Command;
use pretty_assertions::assert_eq;

/// Lo escrito, el comando de Metalmind y el nombre completo en nvim.
const CASES: &[(&str, Command, &str)] = &[
    ("wa", Command::SaveAll, "wall"),
    ("wall", Command::SaveAll, "wall"),
    ("qa", Command::QuitAll, "qall"),
    ("qall", Command::QuitAll, "qall"),
    ("quita", Command::QuitAll, "quitall"),
    ("quitall", Command::QuitAll, "quitall"),
    ("qa!", Command::ForceQuitAll, "qall!"),
    ("quitall!", Command::ForceQuitAll, "quitall!"),
    ("wqa", Command::SaveAllAndQuit, "wqall"),
    ("wqall", Command::SaveAllAndQuit, "wqall"),
    ("xa", Command::SaveAllAndQuit, "xall"),
    ("xall", Command::SaveAllAndQuit, "xall"),
    ("wq", Command::SaveAndQuit, "wq"),
    ("q!", Command::ForceQuit, "quit!"),
    ("w!", Command::ForceSave, "write!"),
    ("wri!", Command::ForceSave, "write!"),
];

#[test]
fn metalmind_reconoce_cada_forma() {
    for &(line, command, _) in CASES {
        let (mut buffer, mut selections) = support::parse("|a");
        let mut vim = support::app_vim();
        let keys = format!(":{line}<CR>");
        let outcome = support::feed(&mut vim, &mut buffer, &mut selections, &keys);
        assert_eq!(outcome, Some(support::dispatched(command)), "{line:?}");
    }
}

#[test]
#[cfg_attr(not(nvim_available), ignore = "nvim no está en el PATH")]
fn nvim_entiende_lo_mismo() {
    let lines: Vec<&str> = CASES.iter().map(|&(line, _, _)| line).collect();
    let expected: Vec<&str> = CASES.iter().map(|&(_, _, name)| name).collect();
    assert_eq!(support::nvim::command_names(&lines), expected);
}
