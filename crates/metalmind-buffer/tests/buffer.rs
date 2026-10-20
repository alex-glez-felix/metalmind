#![allow(
    clippy::unwrap_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

use metalmind_buffer::{
    Bias, Buffer, BufferError, ByteOffset, Edit, HistorySeq, LineAnchor, Selection, Selections,
    Splice,
};
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
fn line_text_leaves_out_the_line_feed() {
    let buffer = Buffer::from("uno\nañb\n");
    assert_eq!(buffer.line_text(0), "uno");
    assert_eq!(buffer.line_text(1), "añb");
    assert_eq!(buffer.line_text(2), "");
    let long = "x".repeat(10_000);
    let buffer = Buffer::from(format!("{long}\n{long}").as_str());
    assert_eq!(buffer.line_text(1), long);
}

#[test]
fn lines_text_joins_whole_lines_without_the_last_line_feed() {
    let buffer = Buffer::from("uno\nañb\n");
    assert_eq!(buffer.lines_text(0..2), "uno\nañb");
    assert_eq!(buffer.lines_text(1..3), "añb\n");
    assert_eq!(buffer.lines_text(2..3), "");
    assert_eq!(buffer.lines_text(1..1), "");
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

/// Que no cambie nada no la vuelve válida: un rango fuera del texto es un
/// bug de quien edita.
#[test]
fn rejects_invalid_no_op_edits() {
    let mut buffer = Buffer::from("aé");
    buffer.begin_transaction(cursor(0)).unwrap();
    assert!(matches!(
        buffer.apply(vec![Edit::insert(at(9), "")]),
        Err(BufferError::OutOfBounds { .. })
    ));
    assert_eq!(
        buffer.apply(vec![Edit::delete(at(2)..at(2))]),
        Err(BufferError::NotCharBoundary(at(2)))
    );
    assert_eq!(
        buffer.apply(vec![Edit::insert(at(0), "x"), Edit::insert(at(9), "")]),
        Err(BufferError::OutOfBounds {
            start: at(9),
            end: at(9),
            len: at(3)
        })
    );
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
fn no_op_edits_leave_no_history() {
    let mut buffer = Buffer::from("abc");
    let version = buffer.version();
    edit(&mut buffer, vec![Edit::delete(at(1)..at(1))]);
    assert_eq!(buffer.version(), version);
    assert!(buffer.undo().unwrap().is_none());
}

/// "abc" → borra la `a` (#1, "bc") → undo → borra la `c` (#2, "ab").
fn branched() -> Buffer {
    let mut buffer = Buffer::from("abc");
    edit(&mut buffer, vec![Edit::delete(at(0)..at(1))]);
    buffer.undo().unwrap();
    edit(&mut buffer, vec![Edit::delete(at(2)..at(3))]);
    buffer
}

fn seq(buffer: &Buffer) -> usize {
    buffer.history_seq().get()
}

#[test]
fn each_saved_transaction_is_a_new_history_seq() {
    let mut buffer = Buffer::from("abc");
    assert_eq!(buffer.history_seq(), HistorySeq::INITIAL);
    edit(&mut buffer, vec![Edit::delete(at(0)..at(1))]);
    assert_eq!(seq(&buffer), 1);
    buffer.undo().unwrap();
    assert_eq!(buffer.history_seq(), HistorySeq::INITIAL);
    edit(&mut buffer, vec![Edit::delete(at(1)..at(1))]);
    assert_eq!(buffer.history_seq(), HistorySeq::INITIAL);
}

#[test]
fn uncommitted_edits_are_visible_until_commit() {
    let mut buffer = Buffer::from("abc");
    buffer.begin_transaction(cursor(0)).unwrap();
    assert!(!buffer.has_uncommitted_edits());
    buffer.apply(vec![Edit::insert(at(0), "x")]).unwrap();
    assert!(buffer.has_uncommitted_edits());
    assert_eq!(buffer.history_seq(), HistorySeq::INITIAL);
    buffer.commit_transaction(cursor(1)).unwrap();
    assert!(!buffer.has_uncommitted_edits());
    assert_eq!(seq(&buffer), 1);
}

#[test]
fn editing_after_undo_keeps_the_undone_branch() {
    let mut buffer = branched();
    assert_eq!(buffer.to_string(), "ab");
    assert!(buffer.redo().unwrap().is_none());

    buffer.earlier(1).unwrap().unwrap();
    assert_eq!((buffer.to_string().as_str(), seq(&buffer)), ("bc", 1));
    buffer.earlier(1).unwrap().unwrap();
    assert_eq!((buffer.to_string().as_str(), seq(&buffer)), ("abc", 0));
    buffer.later(2).unwrap().unwrap();
    assert_eq!((buffer.to_string().as_str(), seq(&buffer)), ("ab", 2));
}

#[test]
fn redo_follows_the_newest_branch() {
    let mut buffer = branched();
    buffer.undo().unwrap();
    buffer.redo().unwrap();
    assert_eq!(buffer.to_string(), "ab");
}

#[test]
fn redo_follows_the_branch_visited_last() {
    let mut buffer = branched();
    buffer.earlier(1).unwrap();
    buffer.undo().unwrap();
    assert_eq!(buffer.to_string(), "abc");
    buffer.redo().unwrap();
    assert_eq!(buffer.to_string(), "bc");
}

#[test]
fn earlier_and_later_stop_at_the_ends() {
    let mut buffer = branched();
    assert!(buffer.later(1).unwrap().is_none());
    buffer.earlier(9).unwrap().unwrap();
    assert_eq!((buffer.to_string().as_str(), seq(&buffer)), ("abc", 0));
    assert!(buffer.earlier(1).unwrap().is_none());
    buffer.later(9).unwrap().unwrap();
    assert_eq!((buffer.to_string().as_str(), seq(&buffer)), ("ab", 2));
}

#[test]
fn earlier_returns_the_last_transaction_it_crossed() {
    let mut buffer = Buffer::from("abc");
    for (start, text) in [(0, "x"), (3, "y")] {
        buffer.begin_transaction(cursor(start)).unwrap();
        buffer.apply(vec![Edit::insert(at(start), text)]).unwrap();
        buffer.commit_transaction(cursor(start + 1)).unwrap();
        buffer.undo().unwrap();
    }
    assert_eq!(buffer.to_string(), "abc");
    buffer.later(2).unwrap();
    let crossed = buffer.earlier(1).unwrap().unwrap();
    assert_eq!(crossed.selections_before(), &cursor(0));
    assert_eq!(buffer.to_string(), "xabc");
}

#[test]
fn history_moves_are_refused_while_a_transaction_is_open() {
    let mut buffer = Buffer::from("abc");
    buffer.begin_transaction(cursor(0)).unwrap();
    let refused = Err(BufferError::TransactionAlreadyOpen);
    assert_eq!(buffer.undo().map(|t| t.is_some()), refused);
    assert_eq!(buffer.redo().map(|t| t.is_some()), refused);
    assert_eq!(buffer.earlier(1).map(|t| t.is_some()), refused);
    assert_eq!(buffer.later(1).map(|t| t.is_some()), refused);
}

#[test]
fn version_grows_with_every_change() {
    let mut buffer = Buffer::from("abc");
    let start = buffer.version();
    edit(&mut buffer, vec![Edit::insert(at(0), "x")]);
    buffer.undo().unwrap();
    assert!(buffer.version() > start + 1);
}

#[test]
fn transactions_in_a_group_are_one_undo_step() {
    let mut buffer = Buffer::from("abc");
    buffer.begin_undo_group();
    edit(&mut buffer, vec![Edit::delete(at(0)..at(1))]);
    edit(&mut buffer, vec![Edit::delete(at(0)..at(1))]);
    buffer.end_undo_group();
    edit(&mut buffer, vec![Edit::insert(at(0), "x")]);
    assert_eq!((buffer.to_string().as_str(), seq(&buffer)), ("xc", 2));
    buffer.undo().unwrap();
    assert_eq!(buffer.to_string(), "c");
    buffer.undo().unwrap();
    assert_eq!(buffer.to_string(), "abc");
    buffer.redo().unwrap();
    assert_eq!(buffer.to_string(), "c");
}

#[test]
fn a_nested_group_is_part_of_the_outer_one() {
    let mut buffer = Buffer::from("abcd");
    buffer.begin_undo_group();
    edit(&mut buffer, vec![Edit::delete(at(0)..at(1))]);
    buffer.begin_undo_group();
    edit(&mut buffer, vec![Edit::delete(at(0)..at(1))]);
    buffer.end_undo_group();
    edit(&mut buffer, vec![Edit::delete(at(0)..at(1))]);
    buffer.end_undo_group();
    edit(&mut buffer, vec![Edit::insert(at(0), "x")]);
    assert_eq!((buffer.to_string().as_str(), seq(&buffer)), ("xd", 2));
    buffer.undo().unwrap();
    buffer.undo().unwrap();
    assert_eq!(buffer.to_string(), "abcd");
}

#[test]
fn moving_through_history_cuts_the_group() {
    let mut buffer = Buffer::from("abc");
    buffer.begin_undo_group();
    edit(&mut buffer, vec![Edit::delete(at(0)..at(1))]);
    buffer.undo().unwrap();
    edit(&mut buffer, vec![Edit::delete(at(2)..at(3))]);
    edit(&mut buffer, vec![Edit::delete(at(1)..at(2))]);
    buffer.end_undo_group();
    assert_eq!(buffer.to_string(), "a");
    buffer.undo().unwrap();
    assert_eq!(buffer.to_string(), "abc");
}

#[test]
fn a_group_closed_mid_transaction_ends_with_it() {
    let mut buffer = Buffer::from("abc");
    buffer.begin_undo_group();
    edit(&mut buffer, vec![Edit::delete(at(0)..at(1))]);
    buffer.begin_transaction(cursor(0)).unwrap();
    buffer.apply(vec![Edit::insert(at(0), "x")]).unwrap();
    buffer.end_undo_group();
    buffer.commit_transaction(cursor(1)).unwrap();
    edit(&mut buffer, vec![Edit::insert(at(0), "y")]);
    buffer.undo().unwrap();
    assert_eq!(buffer.to_string(), "xbc");
    buffer.undo().unwrap();
    assert_eq!(buffer.to_string(), "abc");
}

fn anchored(text: &str, offset: usize) -> (Buffer, LineAnchor) {
    let mut buffer = Buffer::from(text);
    let anchor = buffer.create_anchor(at(offset)).unwrap();
    (buffer, anchor)
}

#[test]
fn an_anchor_follows_lines_inserted_or_deleted_above() {
    let (mut buffer, anchor) = anchored("ab\ncd\nef", 7);
    edit(&mut buffer, vec![Edit::delete(at(0)..at(3))]);
    assert_eq!(buffer.anchor_offset(anchor), Some(at(4)));
    edit(&mut buffer, vec![Edit::insert(at(0), "xyz\n\n")]);
    assert_eq!(buffer.anchor_offset(anchor), Some(at(9)));
}

#[test]
fn an_anchor_keeps_its_column_on_its_own_line() {
    let (mut buffer, anchor) = anchored("abcdef", 4);
    edit(&mut buffer, vec![Edit::insert(at(0), "xy")]);
    assert_eq!(buffer.anchor_offset(anchor), Some(at(4)));
    edit(&mut buffer, vec![Edit::delete(at(1)..at(8))]);
    assert_eq!(buffer.anchor_offset(anchor), Some(at(1)));
}

#[test]
fn an_anchor_never_splits_a_character() {
    let (mut buffer, anchor) = anchored("abcd", 3);
    edit(&mut buffer, vec![Edit::replace(at(0)..at(3), "ñé")]);
    assert_eq!(buffer.anchor_offset(anchor), Some(at(2)));
}

#[test]
fn deleting_its_line_drops_the_anchor_and_undo_brings_it_back() {
    let (mut buffer, anchor) = anchored("ab\ncd\nef", 4);
    edit(&mut buffer, vec![Edit::delete(at(3)..at(6))]);
    assert_eq!(buffer.anchor_offset(anchor), None);
    buffer.undo().unwrap();
    assert_eq!(buffer.anchor_offset(anchor), Some(at(4)));
    buffer.redo().unwrap();
    assert_eq!(buffer.anchor_offset(anchor), None);
}

/// Como `:delm` de vim: el undo de un cambio hecho con la mark puesta la
/// devuelve.
#[test]
fn a_cleared_anchor_comes_back_with_an_undo_from_before() {
    let (mut buffer, anchor) = anchored("ab\ncd", 4);
    edit(&mut buffer, vec![Edit::insert(at(0), "x")]);
    buffer.clear_anchor(anchor).unwrap();
    assert_eq!(buffer.anchor_offset(anchor), None);
    buffer.undo().unwrap();
    assert_eq!(buffer.anchor_offset(anchor), Some(at(4)));
    let mut other = Buffer::from("ab");
    assert_eq!(
        other.clear_anchor(anchor),
        Err(BufferError::UnknownAnchor(anchor))
    );
}

/// Como las marks de vim: cada lado de un cambio guarda dónde estaban.
#[test]
fn undo_and_redo_put_anchors_back_where_they_were() {
    let (mut buffer, anchor) = anchored("ab\ncd", 4);
    edit(&mut buffer, vec![Edit::delete(at(2)..at(3))]);
    assert_eq!(buffer.anchor_offset(anchor), Some(at(3)));
    buffer.undo().unwrap();
    assert_eq!(buffer.anchor_offset(anchor), Some(at(4)));
    buffer.move_anchor(anchor, at(0)).unwrap();
    buffer.redo().unwrap();
    assert_eq!(buffer.anchor_offset(anchor), Some(at(3)));
}

#[test]
fn anchors_belong_to_their_buffer() {
    let (_, anchor) = anchored("ab", 0);
    let mut other = Buffer::from("ab");
    assert_eq!(
        other.move_anchor(anchor, at(0)),
        Err(BufferError::UnknownAnchor(anchor))
    );
    assert_eq!(other.anchor_offset(anchor), None);
    assert!(matches!(
        other.create_anchor(at(3)),
        Err(BufferError::OutOfBounds { .. })
    ));
}

#[test]
fn splices_follow_edits_and_history() {
    let mut buffer = Buffer::from("uno dos tres");
    edit(&mut buffer, vec![Edit::replace(at(4)..at(7), "cuatro")]);
    buffer.undo().unwrap();
    let splices = buffer.take_splices();
    let bytes: Vec<_> = splices
        .iter()
        .map(|s| (s.start, s.removed, s.inserted))
        .collect();
    assert_eq!(bytes, [(at(4), 3, 6), (at(4), 6, 3)]);
    assert_eq!(Splice::map_all(&splices, at(9)), at(9));
    assert_eq!(buffer.take_splices(), []);
}

#[test]
fn splices_since_a_count_survive_taking_them() {
    let mut buffer = Buffer::from("uno dos");
    let start = buffer.splice_count();
    edit(&mut buffer, vec![Edit::replace(at(0)..at(3), "un")]);
    let middle = buffer.splice_count();
    edit(&mut buffer, vec![Edit::insert(at(0), "x")]);
    assert_eq!(buffer.splices_since(start).len(), 2);
    assert_eq!(
        buffer.splices_since(middle),
        &buffer.splices_since(start)[1..]
    );
    let taken = buffer.take_splices();
    assert_eq!(buffer.splice_count(), 2, "sacarlos no los descuenta");
    assert_eq!(buffer.splices_since(start), [], "los sacados no vuelven");
    edit(&mut buffer, vec![Edit::insert(at(0), "y")]);
    assert_eq!(buffer.splices_since(middle).len(), 1);
    assert_eq!(buffer.splices_since(buffer.splice_count()), []);
    assert_eq!(taken.len(), 2);
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

#[derive(Clone, Debug)]
enum Move {
    Edit(Vec<(usize, usize, String)>),
    Undo,
    Redo,
    Earlier(usize),
    Later(usize),
}

fn arbitrary_move() -> impl Strategy<Value = Move> {
    prop_oneof![
        3 => prop::collection::vec((0usize..64, 0usize..64, "[x\nñ]{0,3}"), 1..3)
            .prop_map(Move::Edit),
        1 => Just(Move::Undo),
        1 => Just(Move::Redo),
        1 => (1usize..4).prop_map(Move::Earlier),
        1 => (1usize..4).prop_map(Move::Later),
    ]
}

proptest! {
    #[test]
    fn each_history_seq_always_shows_the_same_text(
        text in "[a-zé\n ]{0,40}",
        moves in prop::collection::vec(arbitrary_move(), 1..40),
    ) {
        let mut buffer = Buffer::from(text.as_str());
        let mut texts = vec![buffer.to_string()];
        for step in &moves {
            let before = seq(&buffer);
            let last = texts.len() - 1;
            match step {
                Move::Edit(picks) => {
                    let edits = arbitrary_edits(&buffer.to_string(), picks);
                    edit(&mut buffer, edits);
                    prop_assert!([before, texts.len()].contains(&seq(&buffer)));
                }
                Move::Undo => {
                    buffer.undo().unwrap();
                }
                Move::Redo => {
                    buffer.redo().unwrap();
                }
                Move::Earlier(steps) => {
                    buffer.earlier(*steps).unwrap();
                    prop_assert_eq!(seq(&buffer), before.saturating_sub(*steps));
                }
                Move::Later(steps) => {
                    buffer.later(*steps).unwrap();
                    prop_assert_eq!(seq(&buffer), (before + steps).min(last));
                }
            }
            if seq(&buffer) == texts.len() {
                texts.push(buffer.to_string());
            }
            prop_assert_eq!(&buffer.to_string(), &texts[seq(&buffer)]);
        }
    }

    /// Mientras nadie las mueva a mano, las anclas dependen solo del estado
    /// del historial, se llegue por donde se llegue, y nunca parten un
    /// carácter.
    #[test]
    fn each_history_seq_always_shows_the_same_anchors(
        text in "[a-zé\n ]{0,40}",
        moves in prop::collection::vec(arbitrary_move(), 1..40),
    ) {
        let mut buffer = Buffer::from(text.as_str());
        let anchors: Vec<LineAnchor> = char_boundaries(&text)
            .into_iter()
            .map(|offset| buffer.create_anchor(at(offset)).unwrap())
            .collect();
        let resolve = |buffer: &Buffer| -> Vec<Option<ByteOffset>> {
            anchors.iter().map(|&anchor| buffer.anchor_offset(anchor)).collect()
        };
        let mut seen = std::collections::HashMap::from([(seq(&buffer), resolve(&buffer))]);
        for step in &moves {
            match step {
                Move::Edit(picks) => {
                    let edits = arbitrary_edits(&buffer.to_string(), picks);
                    edit(&mut buffer, edits);
                }
                Move::Undo => {
                    buffer.undo().unwrap();
                }
                Move::Redo => {
                    buffer.redo().unwrap();
                }
                Move::Earlier(steps) => {
                    buffer.earlier(*steps).unwrap();
                }
                Move::Later(steps) => {
                    buffer.later(*steps).unwrap();
                }
            }
            let now = resolve(&buffer);
            for offset in now.iter().flatten() {
                prop_assert!(buffer.is_char_boundary(*offset));
            }
            let first_visit = seen.entry(seq(&buffer)).or_insert_with(|| now.clone());
            prop_assert_eq!(&*first_visit, &now);
        }
    }

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
            let changes = edits.iter().any(|e| !(e.range.is_empty() && e.text.is_empty()));
            edit(&mut buffer, edits);
            if changes {
                states.push(buffer.to_string());
            }
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

    #[test]
    fn splices_keep_offsets_on_char_boundaries(
        text in "[a-zé\n ]{0,40}",
        moves in prop::collection::vec(arbitrary_move(), 1..20),
    ) {
        let mut buffer = Buffer::from(text.as_str());
        let mut offsets: Vec<ByteOffset> = char_boundaries(&text).into_iter().map(at).collect();
        for step in &moves {
            match step {
                Move::Edit(picks) => {
                    let edits = arbitrary_edits(&buffer.to_string(), picks);
                    edit(&mut buffer, edits);
                }
                Move::Undo => { buffer.undo().unwrap(); }
                Move::Redo => { buffer.redo().unwrap(); }
                Move::Earlier(steps) => { buffer.earlier(*steps).unwrap(); }
                Move::Later(steps) => { buffer.later(*steps).unwrap(); }
            }
            let splices = buffer.take_splices();
            for offset in &mut offsets {
                *offset = Splice::map_all(&splices, *offset);
                prop_assert!(buffer.is_char_boundary(*offset), "{offset} en {:?}", buffer.to_string());
            }
        }
    }
}

#[test]
fn marked_lines_follow_edits_and_undo() {
    let mut buffer = Buffer::from("a\nb\nc\nd");
    buffer.mark_lines([1, 2, 3]);
    edit(&mut buffer, vec![Edit::delete(at(0)..at(2))]);
    assert_eq!(buffer.pop_marked_line(), Some(0));
    edit(&mut buffer, vec![Edit::delete(at(2)..at(4))]);
    buffer.undo().unwrap();
    assert_eq!(buffer.pop_marked_line(), Some(2));
    buffer.clear_marked_lines();
    assert_eq!(buffer.pop_marked_line(), None);
}

#[test]
fn a_snapshot_keeps_the_text_it_was_taken_with() {
    let mut buffer = Buffer::from("uno\ndos");
    let snapshot = buffer.snapshot();
    edit(&mut buffer, vec![Edit::insert(at(0), "x")]);
    assert_eq!(snapshot.to_string(), "uno\ndos");
    assert_eq!(buffer.to_string(), "xuno\ndos");
}
