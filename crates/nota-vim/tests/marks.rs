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

use nota_buffer::{ByteOffset, Edit, Selection, Selections};
use nota_vim::Vim;
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
