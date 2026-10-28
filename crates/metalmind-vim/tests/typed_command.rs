#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

//! Lo que se ofrece al escribir tras `:` fuera de un argumento: el
//! historial con la línea vacía y los nombres de comando que empiezan con
//! lo escrito, tras el rango.

#[allow(
    dead_code,
    reason = "cada binario de test usa solo una parte del soporte compartido"
)]
mod support;

use metalmind_vim::TypedCommand;
use pretty_assertions::assert_eq;

/// Lo escrito y lo que ofrece: los nombres, con el primero el que corre.
const TYPED_NAMES: &[(&str, Option<&[&str]>)] = &[
    ("s", Some(&["substitute", "set", "sort", "split"])),
    ("'<,'>s", Some(&["substitute", "set", "sort", "split"])),
    ("%  so", Some(&["sort"])),
    ("  :wa", Some(&["wall"])),
    ("w!", Some(&["write!"])),
    ("foc", Some(&["focus"])),
    ("focus", None),
    ("focus ", None),
    ("%s/a", None),
    ("12", None),
    ("zz", None),
];

fn names(typed: Option<TypedCommand>) -> Option<Vec<&'static str>> {
    match typed? {
        TypedCommand::Name { names, .. } => Some(names.iter().map(|name| name.name).collect()),
        TypedCommand::Empty { .. } => None,
    }
}

#[test]
fn ofrece_los_nombres_que_empiezan_con_lo_escrito() {
    for &(line, expected) in TYPED_NAMES {
        let mut editor = support::Editor::app("|a");
        editor.keys(&format!(":{line}"));
        let typed = editor.vim.typed_command();
        if let Some(TypedCommand::Name { range, .. }) = &typed {
            assert_eq!(range.end, line.len(), "{line:?}");
        }
        assert_eq!(names(typed), expected.map(<[&str]>::to_vec), "{line:?}");
    }
}

#[test]
fn con_la_linea_vacia_ofrece_el_historial_de_lo_mas_nuevo_a_lo_mas_viejo() {
    let mut editor = support::Editor::app("|a");
    editor.keys(":");
    assert_eq!(editor.vim.typed_command(), None, "sin historial, nada");
    editor.keys("noh<CR>:12<CR>:noh<CR>:");
    assert_eq!(
        editor.vim.typed_command(),
        Some(TypedCommand::Empty {
            history: vec!["noh".to_owned(), "12".to_owned()],
        })
    );
    editor.keys("<Esc>/");
    assert_eq!(editor.vim.typed_command(), None, "la búsqueda no");
}

#[test]
fn completar_cambia_el_nombre_o_escribe_la_linea_del_historial() {
    let mut editor = support::Editor::app("|a");
    editor.keys(":'<,'>s");
    editor.vim.complete_command("sort");
    assert_eq!(editor.vim.command_line(), Some("'<,'>sort"));
    editor.keys("<Esc>:fo");
    editor.vim.complete_command("focus ");
    assert_eq!(editor.vim.command_line(), Some("focus "));
    editor.keys("<Esc>:noh<CR>:");
    editor.vim.complete_command("noh");
    assert_eq!(editor.vim.command_line(), Some("noh"));
    editor.vim.complete_command("nohlsearch");
    assert_eq!(editor.vim.command_line(), Some("nohlsearch"));
    editor.vim.complete_command("sort");
    assert_eq!(
        editor.vim.command_line(),
        Some("nohlsearch"),
        "ya es el único nombre: no hay nada que ofrecer"
    );
}
