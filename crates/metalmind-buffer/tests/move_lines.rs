#![allow(
    clippy::unwrap_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

//! `Buffer::move_lines`: el texto cambia de lugar y lo que sigue a las
//! líneas va con él, como `:m` de vim.

use std::ops::RangeInclusive;

use metalmind_buffer::{Buffer, BufferError, ByteOffset, LineAnchor, Point, Selection, Selections};
use pretty_assertions::assert_eq;
use proptest::prelude::*;

fn cursor() -> Selections {
    Selections::single(Selection::cursor(ByteOffset::new(0)))
}

fn move_lines(buffer: &mut Buffer, lines: RangeInclusive<usize>, below: usize) {
    buffer.begin_transaction(cursor()).unwrap();
    buffer.move_lines(lines, below).unwrap();
    buffer.commit_transaction(cursor()).unwrap();
}

/// Un ancla en la columna 1 de cada línea de `buffer`.
fn anchor_every_line(buffer: &mut Buffer) -> Vec<LineAnchor> {
    (0..buffer.line_count())
        .map(|line| {
            let offset = buffer.offset_at_point(Point::new(line, 1));
            buffer.create_anchor(offset).unwrap()
        })
        .collect()
}

fn anchored_lines(buffer: &Buffer, anchors: &[LineAnchor]) -> Vec<String> {
    anchors
        .iter()
        .map(|&anchor| {
            let offset = buffer.anchor_offset(anchor).unwrap();
            buffer.line_text(buffer.line_of(offset)).into_owned()
        })
        .collect()
}

#[test]
fn moves_the_text_like_vim() {
    let cases = [
        ("a\nb\nc\nd", 1..=1, 0, "b\na\nc\nd"),
        ("a\nb\nc\nd\ne", 0..=1, 5, "c\nd\ne\na\nb"),
        ("a\nb\nc\nd\ne", 3..=4, 1, "a\nd\ne\nb\nc"),
        ("a\nb\nc", 2..=2, 0, "c\na\nb"),
        ("a\nb\nc", 0..=0, 3, "b\nc\na"),
        ("a\nb\n", 0..=0, 3, "b\n\na"),
    ];
    for (before, lines, below, after) in cases {
        let mut buffer = Buffer::from(before);
        move_lines(&mut buffer, lines.clone(), below);
        assert_eq!(buffer.to_string(), after, "{lines:?} debajo de {below}");
    }
}

#[test]
fn anchors_go_with_their_lines_and_undo_brings_them_back() {
    let mut buffer = Buffer::from("l0\nl1\nl2\nl3\nl4");
    let anchors = anchor_every_line(&mut buffer);
    let before = anchored_lines(&buffer, &anchors);
    move_lines(&mut buffer, 3..=4, 1);
    assert_eq!(buffer.to_string(), "l0\nl3\nl4\nl1\nl2");
    assert_eq!(anchored_lines(&buffer, &anchors), before);
    assert_eq!(
        buffer.point_at(buffer.anchor_offset(anchors[3]).unwrap()),
        Ok(Point::new(1, 1))
    );
    buffer.undo().unwrap();
    let lines: Vec<_> = anchors
        .iter()
        .map(|&anchor| buffer.point_at(buffer.anchor_offset(anchor).unwrap()))
        .collect();
    let expected: Vec<_> = (0..5).map(|line| Ok(Point::new(line, 1))).collect();
    assert_eq!(lines, expected);
}

#[test]
fn splices_carry_the_move_for_positions_kept_outside() {
    let mut buffer = Buffer::from("a\nb\nc\nd");
    buffer.take_splices();
    move_lines(&mut buffer, 1..=1, 0);
    let splices = buffer.take_splices();
    let follow = |line| {
        splices
            .iter()
            .fold(Point::new(line, 0), |point, splice| {
                splice.lines.keep(point)
            })
            .line
    };
    assert_eq!((0..4).map(follow).collect::<Vec<_>>(), [1, 0, 2, 3]);
}

/// Como `:m` de vim, que mete copias: las marcadas que se mueven se pierden.
#[test]
fn marked_lines_that_move_lose_their_mark() {
    let mut buffer = Buffer::from("a\nb\nc\nd");
    buffer.mark_lines([0, 2, 3]);
    move_lines(&mut buffer, 2..=2, 0);
    assert_eq!(buffer.to_string(), "c\na\nb\nd");
    let marked: Vec<_> = std::iter::from_fn(|| buffer.pop_marked_line()).collect();
    assert_eq!(marked, [1, 3]);
}

#[test]
fn staying_put_changes_nothing() {
    let mut buffer = Buffer::from("a\nb\nc");
    let seq = buffer.history_seq();
    move_lines(&mut buffer, 1..=1, 1);
    move_lines(&mut buffer, 1..=1, 2);
    assert_eq!(buffer.to_string(), "a\nb\nc");
    assert_eq!(buffer.history_seq(), seq);
}

#[test]
fn rejects_moves_into_themselves_or_out_of_the_text() {
    let mut buffer = Buffer::from("a\nb\n");
    buffer.begin_transaction(cursor()).unwrap();
    for (lines, below) in [(1..=2, 2), (0..=2, 1), (0..=3, 0), (0..=0, 4)] {
        let (first, last) = (*lines.start(), *lines.end());
        assert_eq!(
            buffer.move_lines(lines, below),
            Err(BufferError::InvalidMove { first, last, below })
        );
    }
    assert_eq!(buffer.to_string(), "a\nb\n");
}

proptest! {
    /// Cada ancla sigue en una línea con el mismo texto que antes, y deshacer
    /// devuelve el texto.
    #[test]
    fn every_anchor_keeps_its_line_text(
        count in 1..12usize,
        first in 0..12usize,
        len in 0..12usize,
        below in 0..13usize,
    ) {
        let text: Vec<String> = (0..count).map(|line| format!("l{line}")).collect();
        let text = text.join("\n");
        let first = first.min(count - 1);
        let last = (first + len).min(count - 1);
        prop_assume!(below <= count && !(first + 1..=last).contains(&below));
        let mut buffer = Buffer::from(text.as_str());
        let anchors = anchor_every_line(&mut buffer);
        let before = anchored_lines(&buffer, &anchors);
        move_lines(&mut buffer, first..=last, below);
        prop_assert_eq!(anchored_lines(&buffer, &anchors), before);
        buffer.undo().unwrap();
        prop_assert_eq!(buffer.to_string(), text);
    }
}
