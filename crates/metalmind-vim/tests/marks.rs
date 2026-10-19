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

use metalmind_buffer::{Buffer, ByteOffset, Edit, Selection, Selections};
use metalmind_vim::{FileId, Marks, PlacedMark, Vim};
use pretty_assertions::assert_eq;

/// Una mark es una sola posición: saltar a ella deja un solo cursor.
#[test]
fn jumping_to_a_mark_leaves_one_cursor() {
    let state = support::run_engine("|ab\n|cd\nef", "majl'a");
    assert_eq!(state.text, "|ab\ncd\nef");
}

#[test]
fn marks_follow_edits_made_outside_the_engine() {
    let (mut buffer, mut selections) = support::parse("ab\nc|d");
    let mut vim = Vim::new();
    support::feed(&mut vim, &mut buffer, &mut selections, "ma");
    buffer.begin_transaction(selections.clone()).unwrap();
    buffer
        .apply(vec![Edit::insert(ByteOffset::ZERO, "pegado\n")])
        .unwrap();
    buffer.commit_transaction(selections.clone()).unwrap();
    selections = Selections::single(Selection::cursor(ByteOffset::ZERO));
    vim.cursor_moved();
    support::feed(&mut vim, &mut buffer, &mut selections, "`a");
    let heads = selections.iter().map(|s| s.head);
    assert_eq!(support::render(&buffer, heads), "pegado\nab\nc|d");
}

/// Las marks puestas, como las ve quien aloja el motor: con las del buffer
/// devueltas al documento.
fn placed(vim: &mut Vim, buffer: &Buffer, file: FileId) -> Vec<(char, usize)> {
    vim.catch_up(buffer);
    let mut marks = Marks::default();
    vim.swap_marks(&mut marks);
    let placed = vim
        .placed_marks(&marks, file, buffer)
        .map(|PlacedMark { name, line }| (name, line))
        .collect();
    vim.swap_marks(&mut marks);
    placed
}

#[test]
fn the_placed_marks_follow_edits_and_delm() {
    let (mut buffer, mut selections) = support::parse("|a\nb\nc\nd");
    let mut vim = Vim::new();
    support::feed(&mut vim, &mut buffer, &mut selections, "jmBmzggmajjmA");
    let file = FileId::default();
    assert_eq!(
        placed(&mut vim, &buffer, file),
        [('a', 0), ('z', 1), ('A', 2), ('B', 1)]
    );
    support::feed(
        &mut vim,
        &mut buffer,
        &mut selections,
        "ggOx<Esc>:delm z<CR>",
    );
    assert_eq!(
        placed(&mut vim, &buffer, file),
        [('a', 1), ('A', 3), ('B', 2)]
    );
    assert_eq!(
        placed(&mut vim, &buffer, FileId::new(7)),
        [('a', 1)],
        "las A–Z de otro archivo no caen en este buffer"
    );
    support::feed(&mut vim, &mut buffer, &mut selections, "u");
    assert_eq!(
        placed(&mut vim, &buffer, file),
        [('a', 0), ('z', 1), ('A', 2), ('B', 1)],
        "el undo devuelve la z, como en vim, y las A–Z siguen al texto"
    );
}
