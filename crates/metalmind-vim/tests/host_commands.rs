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
        let mut editor = support::Editor::app("|a");
        let keys = format!(":{line}<CR>");
        let outcome = editor.keys(&keys);
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

/// El comando, el texto de la palabra y si empieza literal.
type Completed = (Command, &'static str, bool);

/// Lo escrito tras `:`, y el comando y la palabra que se completarían.
const TYPED_ARGUMENTS: &[(&str, Option<Completed>)] = &[
    ("focus #ru", Some((Command::Focus, "#ru", false))),
    (
        "focus proyectos/ -#ar",
        Some((Command::Focus, "-#ar", false)),
    ),
    (r"focus Mis\ no", Some((Command::Focus, "Mis no", false))),
    (r#"focus "Mis no"#, Some((Command::Focus, "Mis no", true))),
    ("focus #rust ", Some((Command::Focus, "", false))),
    ("  :e  not", Some((Command::Edit, "not", false))),
    ("focus", None),
    ("wa x", None),
    ("nada x", None),
];

#[test]
fn sabe_que_palabra_del_argumento_se_escribe() {
    for &(line, expected) in TYPED_ARGUMENTS {
        let mut editor = support::Editor::app("|a");
        editor.keys(&format!(":{line}"));
        let typed = editor.vim.typed_argument().map(|typed| {
            let word = typed.word;
            assert_eq!(word.range.end, line.len(), "{line:?}");
            (typed.action, word.text, word.literal_start)
        });
        let expected = expected.map(|(command, text, literal)| {
            (
                metalmind_vim::ActionName::new(command.name()),
                text.to_owned(),
                literal,
            )
        });
        assert_eq!(typed, expected, "{line:?}");
    }
}

#[test]
fn completar_cambia_solo_la_palabra_que_se_escribe() {
    let mut editor = support::Editor::app("|a");
    editor.keys(r#":focus proyectos/ "Mis no"#);
    editor.vim.complete_argument(r"Mis\ notas/");
    assert_eq!(
        editor.vim.command_line(),
        Some(r"focus proyectos/ Mis\ notas/")
    );
    editor.keys(" ");
    editor.vim.complete_argument("#Rust");
    assert_eq!(
        editor.vim.command_line(),
        Some(r"focus proyectos/ Mis\ notas/ #Rust")
    );
}

#[test]
fn sin_argumento_que_completar_no_cambia_nada() {
    let mut editor = support::Editor::app("|a");
    editor.keys("/focus #r");
    editor.vim.complete_argument("#Rust");
    assert_eq!(editor.vim.command_line(), Some("focus #r"));
    assert_eq!(editor.vim.typed_argument(), None);
}
