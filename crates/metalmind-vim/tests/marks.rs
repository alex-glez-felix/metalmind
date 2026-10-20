#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

//! Lo de las marks que nvim no puede mostrar en la tabla de `vim.rs`.

#[allow(
    dead_code,
    reason = "cada binario de test usa solo una parte del soporte compartido"
)]
mod support;

use metalmind_buffer::{ByteOffset, Edit, Selection, Selections};
use metalmind_vim::{FileId, PlacedMark};
use pretty_assertions::assert_eq;

/// Una mark es una sola posición: saltar a ella deja un solo cursor.
#[test]
fn jumping_to_a_mark_leaves_one_cursor() {
    let state = support::run_engine("|ab\n|cd\nef", "majl'a");
    assert_eq!(state.text, "|ab\ncd\nef");
}

#[test]
fn marks_follow_edits_made_outside_the_engine() {
    let mut editor = support::Editor::new("ab\nc|d");
    editor.keys("ma");
    let selections = editor.selections.clone();
    editor.buffer.begin_transaction(selections.clone()).unwrap();
    editor
        .buffer
        .apply(vec![Edit::insert(ByteOffset::ZERO, "pegado\n")])
        .unwrap();
    editor.buffer.commit_transaction(selections).unwrap();
    editor.selections = Selections::single(Selection::cursor(ByteOffset::ZERO));
    editor.vim.cursor_moved();
    editor.keys("`a");
    assert_eq!(editor.cursors(), "pegado\nab\nc|d");
}

/// Lo de fuera también lo siguen las marks `A`–`Z`: el motor sabe qué
/// reemplazos del buffer ya siguió, aunque nadie se lo diga. Solo los que
/// siguen en el buffer: los que alguien saca antes con `take_splices` no los ve.
#[test]
fn file_marks_follow_edits_made_outside_the_engine() {
    let mut editor = support::Editor::new("ab\nc|d");
    editor.keys("mA");
    let selections = editor.selections.clone();
    editor.buffer.begin_transaction(selections.clone()).unwrap();
    editor
        .buffer
        .apply(vec![Edit::insert(ByteOffset::ZERO, "pegado\n")])
        .unwrap();
    editor.buffer.commit_transaction(selections).unwrap();
    editor.selections = Selections::single(Selection::cursor(ByteOffset::ZERO));
    editor.vim.cursor_moved();
    assert_eq!(placed(&editor, FileId::default()), [('A', 2)]);
    editor.keys("`A");
    assert_eq!(editor.cursors(), "pegado\nab\nc|d");
}

/// Las marks puestas, como las ve quien aloja el motor.
fn placed(editor: &support::Editor, file: FileId) -> Vec<(char, usize)> {
    editor
        .vim
        .placed_marks(&editor.doc, file, &editor.buffer)
        .map(|PlacedMark { name, line }| (name, line))
        .collect()
}

#[test]
fn the_placed_marks_follow_edits_and_delm() {
    let mut editor = support::Editor::new("|a\nb\nc\nd");
    editor.keys("jmBmzggmajjmA");
    let file = FileId::default();
    assert_eq!(
        placed(&editor, file),
        [('a', 0), ('z', 1), ('A', 2), ('B', 1)]
    );
    editor.keys("ggOx<Esc>:delm z<CR>");
    assert_eq!(placed(&editor, file), [('a', 1), ('A', 3), ('B', 2)]);
    assert_eq!(
        placed(&editor, FileId::new(7)),
        [('a', 1)],
        "las A–Z de otro archivo no caen en este buffer"
    );
    editor.keys("u");
    assert_eq!(
        placed(&editor, file),
        [('a', 0), ('z', 1), ('A', 2), ('B', 1)],
        "el undo devuelve la z, como en vim, y las A–Z siguen al texto"
    );
}
