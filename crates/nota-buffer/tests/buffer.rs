#![allow(
    clippy::unwrap_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

use nota_buffer::{Bias, Buffer, BufferError, ByteOffset, Edit, Selection, Selections};
use pretty_assertions::assert_eq;
use proptest::prelude::*;

fn at(offset: usize) -> ByteOffset {
    ByteOffset::new(offset)
}

fn cursor(offset: usize) -> Selections {
    Selections::single(Selection::cursor(at(offset)))
}

fn edit(buffer: &mut Buffer, edits: Vec<Edit>) {
    buffer.begin_transaction(cursor(0)).unwrap();
    buffer.apply(edits).unwrap();
    buffer.commit_transaction(cursor(0)).unwrap();
}

#[test]
fn lines_split_only_on_line_feed() {
    let buffer = Buffer::from("uno\r\ndos\u{2028}tres\n");
    assert_eq!(buffer.line_count(), 3);
    assert_eq!(buffer.line_end(0), at(4));
    assert_eq!(buffer.line_start(1), at(5));
    assert_eq!(buffer.line_end(2), buffer.len());
}

#[test]
fn columns_count_characters_not_bytes() {
    let buffer = Buffer::from("añb\ncd");
    assert_eq!(buffer.column_of(at(3)), 2);
    assert_eq!(buffer.offset_at_column(0, 2), at(3));
    assert_eq!(buffer.offset_at_column(0, 99), at(4));
    assert_eq!(buffer.offset_at_column(1, 1), at(6));
}

#[test]
fn walks_multibyte_characters() {
    let buffer = Buffer::from("aé");
    assert_eq!(buffer.next_boundary(at(1)), at(3));
    assert_eq!(buffer.prev_boundary(at(3)), at(1));
    assert_eq!(buffer.char_at(at(2)), None);
    assert_eq!(buffer.char_before(at(3)), Some('é'));
}

#[test]
fn applies_a_batch_in_original_coordinates() {
    let mut buffer = Buffer::from("hello world");
    buffer.begin_transaction(cursor(0)).unwrap();
    let map = buffer
        .apply(vec![
            Edit::replace(at(6)..at(11), "nota"),
            Edit::delete(at(0)..at(1)),
            Edit::insert(at(5), ","),
        ])
        .unwrap();
    buffer.commit_transaction(cursor(0)).unwrap();
    assert_eq!(buffer.to_string(), "ello, nota");
    assert_eq!(map.map(at(6), Bias::Before), at(6));
}

#[test]
fn rejects_invalid_edits() {
    let mut buffer = Buffer::from("aé");
    assert_eq!(
        buffer.apply(vec![Edit::insert(at(0), "x")]),
        Err(BufferError::NoOpenTransaction)
    );
    buffer.begin_transaction(cursor(0)).unwrap();
    assert_eq!(
        buffer.apply(vec![Edit::delete(at(0)..at(2))]),
        Err(BufferError::NotCharBoundary(at(2)))
    );
    assert!(matches!(
        buffer.apply(vec![Edit::delete(at(0)..at(9))]),
        Err(BufferError::OutOfBounds { .. })
    ));
    assert!(matches!(
        buffer.apply(vec![Edit::delete(at(0)..at(1)), Edit::insert(at(0), "x")]),
        Err(BufferError::OverlappingEdits { .. })
    ));
    assert_eq!(buffer.to_string(), "aé");
}

#[test]
fn a_transaction_is_one_undo_step() {
    let mut buffer = Buffer::from("abc");
    buffer.begin_transaction(cursor(1)).unwrap();
    buffer.apply(vec![Edit::insert(at(1), "x")]).unwrap();
    buffer.apply(vec![Edit::insert(at(2), "y")]).unwrap();
    buffer.commit_transaction(cursor(3)).unwrap();
    assert_eq!(buffer.to_string(), "axybc");

    let undone = buffer.undo().unwrap().unwrap();
    assert_eq!(undone.selections_before(), &cursor(1));
    assert_eq!(undone.selections_after(), &cursor(3));
    assert_eq!(buffer.to_string(), "abc");

    buffer.redo().unwrap().unwrap();
    assert_eq!(buffer.to_string(), "axybc");
}

#[test]
fn empty_transactions_leave_no_history() {
    let mut buffer = Buffer::from("abc");
    buffer.begin_transaction(cursor(0)).unwrap();
    buffer.commit_transaction(cursor(0)).unwrap();
    assert!(buffer.undo().unwrap().is_none());
}

#[test]
fn new_edits_clear_redo() {
    let mut buffer = Buffer::from("abc");
    edit(&mut buffer, vec![Edit::delete(at(0)..at(1))]);
    buffer.undo().unwrap();
    edit(&mut buffer, vec![Edit::delete(at(2)..at(3))]);
    assert!(buffer.redo().unwrap().is_none());
    assert_eq!(buffer.to_string(), "ab");
}

#[test]
fn undo_is_refused_while_a_transaction_is_open() {
    let mut buffer = Buffer::from("abc");
    buffer.begin_transaction(cursor(0)).unwrap();
    assert_eq!(
        buffer.undo().map(|t| t.is_some()),
        Err(BufferError::TransactionAlreadyOpen)
    );
}

#[test]
fn version_grows_with_every_change() {
    let mut buffer = Buffer::from("abc");
    let start = buffer.version();
    edit(&mut buffer, vec![Edit::insert(at(0), "x")]);
    buffer.undo().unwrap();
    assert!(buffer.version() > start + 1);
}

fn char_boundaries(text: &str) -> Vec<usize> {
    text.char_indices()
        .map(|(i, _)| i)
        .chain([text.len()])
        .collect()
}

fn arbitrary_edits(text: &str, picks: &[(usize, usize, String)]) -> Vec<Edit> {
    let bounds = char_boundaries(text);
    let mut points: Vec<usize> = picks
        .iter()
        .flat_map(|(a, b, _)| [bounds[a % bounds.len()], bounds[b % bounds.len()]])
        .collect();
    points.sort_unstable();
    points
        .as_chunks::<2>()
        .0
        .iter()
        .zip(picks)
        .map(|([start, end], (_, _, text))| Edit::replace(at(*start)..at(*end), text.clone()))
        .collect()
}

proptest! {
    #[test]
    fn undo_restores_and_redo_reapplies(
        text in "[a-zé\n ]{0,40}",
        batches in prop::collection::vec(
            prop::collection::vec((0usize..64, 0usize..64, "[x\nñ]{0,3}"), 1..4),
            1..5,
        ),
    ) {
        let mut buffer = Buffer::from(text.as_str());
        let mut states = vec![buffer.to_string()];
        for picks in &batches {
            let edits = arbitrary_edits(&buffer.to_string(), picks);
            edit(&mut buffer, edits);
            states.push(buffer.to_string());
        }
        for expected in states.iter().rev().skip(1) {
            buffer.undo().unwrap();
            prop_assert_eq!(&buffer.to_string(), expected);
        }
        for expected in states.iter().skip(1) {
            buffer.redo().unwrap();
            prop_assert_eq!(&buffer.to_string(), expected);
        }
    }
}
