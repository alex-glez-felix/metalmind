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

use metalmind_keymap::{Command, WindowCommand};
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
type Completed = (&'static str, &'static str, bool);

/// Lo escrito tras `:`, y el comando y la palabra que se completarían.
const TYPED_ARGUMENTS: &[(&str, Option<Completed>)] = &[
    (
        "focus #ru",
        Some((WindowCommand::Focus.name(), "#ru", false)),
    ),
    (
        "focus proyectos/ -#ar",
        Some((WindowCommand::Focus.name(), "-#ar", false)),
    ),
    (
        r"focus Mis\ no",
        Some((WindowCommand::Focus.name(), "Mis no", false)),
    ),
    (
        r#"focus "Mis no"#,
        Some((WindowCommand::Focus.name(), "Mis no", true)),
    ),
    (
        "focus #rust ",
        Some((WindowCommand::Focus.name(), "", false)),
    ),
    ("  :e  not", Some((Command::Edit.name(), "not", false))),
    (r"e Mis\ no", Some((Command::Edit.name(), "Mis no", false))),
    ("e Mis no", Some((Command::Edit.name(), "Mis no", false))),
    (r#"e "Mis"#, Some((Command::Edit.name(), "\"Mis", false))),
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
                metalmind_vim::ActionName::new(command),
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

/// El argumento de `:e` y la ruta que lee Metalmind.
const FILE_NAMES: &[(&str, &str)] = &[
    ("notas/a.md", "notas/a.md"),
    (r"Mis\ notas/a.md", "Mis notas/a.md"),
    ("a b.md", "a b.md"),
    (r"x\ay", "xay"),
    (r"x\\y", r"x\y"),
    (r#"x\"y"#, "x\"y"),
    (r"x\#y", "x#y"),
    (r"x\", r"x\"),
    ("lead.md  ", "lead.md"),
    (r"a\ ", "a "),
    ("\u{a0}x", "\u{a0}x"),
    ("\tlead.md\t", "lead.md"),
    ("x\u{a0}", "x\u{a0}"),
    (r"a\ b\ c", "a b c"),
];

#[test]
fn lee_la_ruta_de_e_como_vim() {
    for &(argument, expected) in FILE_NAMES {
        assert_eq!(metalmind_vim::file_name(argument), expected, "{argument:?}");
    }
}

/// De la línea de `:` a la ruta: lo que `parse` deja del argumento también
/// se lee como en nvim, con el espacio escapado del final.
#[test]
fn la_linea_de_e_llega_entera_a_la_ruta() {
    for &(argument, expected) in FILE_NAMES {
        let mut editor = support::Editor::app("|a");
        let outcome = editor.keys(&format!(":e {argument}<CR>"));
        let Some(metalmind_vim::KeyOutcome::Dispatch(invocation)) = outcome else {
            panic!("{argument:?}: {outcome:?}");
        };
        let read = invocation.argument.as_deref().map(metalmind_vim::file_name);
        assert_eq!(read.as_deref(), Some(expected), "{argument:?}");
    }
}

#[test]
#[cfg_attr(not(nvim_available), ignore = "nvim no está en el PATH")]
fn nvim_abre_la_misma_ruta() {
    let arguments: Vec<&str> = FILE_NAMES.iter().map(|&(argument, _)| argument).collect();
    let expected: Vec<&str> = FILE_NAMES.iter().map(|&(_, name)| name).collect();
    assert_eq!(support::nvim::file_names(&arguments), expected);
}

/// En `:e` se completa la ruta entera, aunque lleve espacios sin escapar.
#[test]
fn completar_en_e_cambia_la_ruta_entera() {
    let mut editor = support::Editor::app("|a");
    editor.keys(":e Mis no");
    editor.vim.complete_argument(r"Mis\ notas/rust.md");
    assert_eq!(editor.vim.command_line(), Some(r"e Mis\ notas/rust.md"));
}
