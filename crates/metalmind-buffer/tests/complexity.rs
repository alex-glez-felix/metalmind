//! Una edición no depende del tamaño del buffer, y un lote de ediciones (un
//! `:s`, un cursor por línea) es lineal en el lote. Ver `docs/performance.md`.

#![allow(
    clippy::unwrap_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

use metalmind_buffer::{Buffer, ByteOffset, Edit, Selection, Selections};
use metalmind_perf::{assert_flat, assert_linear, note};

fn cursor() -> Selections {
    Selections::single(Selection::cursor(ByteOffset::new(0)))
}

fn edit(buffer: &mut Buffer, edits: Vec<Edit>) {
    buffer.begin_transaction(cursor()).unwrap();
    buffer.apply(edits).unwrap();
    buffer.commit_transaction(cursor()).unwrap();
}

fn buffer(lines: usize) -> Buffer {
    Buffer::from(note(lines).as_str())
}

fn one_insert_per_line(buffer: &Buffer) -> Vec<Edit> {
    (0..buffer.line_count())
        .map(|line| Edit::insert(buffer.line_start(line), "> "))
        .collect()
}

#[test]
fn typing_does_not_depend_on_the_buffer_size() {
    assert_flat(2_000, buffer, |mut buffer| {
        let at = buffer.line_start(buffer.line_count() / 2);
        for _ in 0..500 {
            edit(&mut buffer, vec![Edit::insert(at, "x")]);
        }
        buffer
    });
}

#[test]
fn a_batch_of_edits_is_linear() {
    assert_linear(
        300,
        |lines| {
            let buffer = buffer(lines);
            let edits = one_insert_per_line(&buffer);
            (buffer, edits)
        },
        |(mut buffer, edits)| {
            edit(&mut buffer, edits);
            buffer
        },
    );
}

#[test]
fn undoing_a_batch_is_linear() {
    assert_linear(
        400,
        |lines| {
            let mut buffer = buffer(lines);
            let edits = one_insert_per_line(&buffer);
            edit(&mut buffer, edits);
            buffer
        },
        |mut buffer| {
            buffer.undo().unwrap();
            buffer
        },
    );
}
