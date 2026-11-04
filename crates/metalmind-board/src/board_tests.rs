use pretty_assertions::assert_eq;
use rstest::rstest;
use similar::{ChangeTag, TextDiff};

use super::*;
use crate::edge::{Route, Side, Tip};
use crate::node::{NodeKind, Shape};

/// Como lo guarda Obsidian: sin extensiones y un grupo con fondo.
const FROM_OBSIDIAN: &str = include_str!("../tests/fixtures/obsidian.canvas");
const EXAMPLE: &str = include_str!("../tests/fixtures/arquitectura.canvas");

#[test]
fn reads_what_obsidian_writes_with_json_canvas_defaults() {
    let board = Board::parse(FROM_OBSIDIAN).unwrap();
    assert_eq!(board.nodes.len(), 4);
    assert_eq!(board.edges[0].from.side, Some(Side::Right));
    assert_eq!(board.edges[0].from.tip, Tip::None);
    assert_eq!(board.edges[0].to.tip, Tip::Arrow);
    assert_eq!(board.edges[0].route, Route::Curve);
    assert!(matches!(
        &board.nodes[0].kind,
        NodeKind::Text {
            shape: Shape::Rectangle,
            ..
        }
    ));
}

#[rstest]
#[case("")]
#[case(" \n")]
#[case("{}")]
#[case("{\"nodes\":[],\"edges\":[]}")]
fn an_empty_file_is_an_empty_board(#[case] text: &str) {
    assert_eq!(Board::parse(text).unwrap(), Board::default());
}

#[test]
fn an_empty_board_is_written_without_blank_lines() {
    assert_eq!(
        Board::default().write(),
        "{\n\t\"nodes\":[],\n\t\"edges\":[]\n}\n"
    );
}

#[test]
fn moving_a_box_changes_only_its_line() {
    let mut board = Board::parse(EXAMPLE).unwrap();
    let before = board.write();
    let moved = board.nodes.iter_mut().find(|n| n.id == "formato").unwrap();
    moved.x += 40;
    moved.y -= 20;
    let after = board.write();
    let diff = TextDiff::from_lines(&before, &after);
    let inserted: Vec<_> = diff
        .iter_all_changes()
        .filter(|change| change.tag() == ChangeTag::Insert)
        .collect();
    assert_eq!(inserted.len(), 1, "{inserted:?}");
    assert!(
        inserted[0]
            .value()
            .trim()
            .starts_with(r#"{"id":"formato","type":"text","x":480,"y":160,"#),
    );
}

#[test]
fn a_loop_does_not_keep_a_route() {
    let text = FROM_OBSIDIAN.replace(
        r#""toNode":"b2","toSide":"left""#,
        r#""toNode":"a1","route":"elbow""#,
    );
    let board = Board::parse(&text).unwrap();
    assert_eq!(board.edges[0].route, Route::Curve);
    assert!(!board.write().contains("\"route\""));
}

#[test]
fn two_elements_with_the_same_id_are_an_error() {
    let node = FROM_OBSIDIAN.replace(r#""id":"b2","type":"file""#, r#""id":"a1","type":"file""#);
    assert!(matches!(Board::parse(&node), Err(FormatError::DuplicateId(id)) if id == "a1"));
    let edge = FROM_OBSIDIAN.replace(r#""id":"e1""#, r#""id":"c3""#);
    assert!(matches!(Board::parse(&edge), Err(FormatError::DuplicateId(id)) if id == "c3"));
}

#[test]
fn an_edge_to_a_missing_node_is_an_error() {
    let text = FROM_OBSIDIAN.replace(r#""toNode":"b2""#, r#""toNode":"zz""#);
    assert!(matches!(
        Board::parse(&text),
        Err(FormatError::DanglingEdge { .. })
    ));
}

#[test]
fn unknown_types_and_missing_fields_are_errors() {
    let unknown = FROM_OBSIDIAN.replace(r#""type":"text""#, r#""type":"video""#);
    assert!(matches!(
        Board::parse(&unknown),
        Err(FormatError::UnknownType { .. })
    ));
    let missing = FROM_OBSIDIAN.replace(r#""file":"ideas/pizarra.md","#, "");
    assert!(matches!(
        Board::parse(&missing),
        Err(FormatError::Missing { field: "file", .. })
    ));
}

#[rstest]
#[case("0")]
#[case("-1")]
#[case("1e40")]
fn a_stroke_width_that_cannot_be_painted_is_an_error(#[case] width: &str) {
    let text = format!(
        r#"{{"nodes":[{{"id":"t","type":"stroke","x":0,"y":0,"width":9,"height":9,"strokeWidth":{width},"points":"0,0"}}]}}"#
    );
    assert!(matches!(
        Board::parse(&text),
        Err(FormatError::StrokeWidth { .. })
    ));
}

#[test]
fn points_that_do_not_read_are_an_error_with_the_id() {
    let text = r#"{"nodes":[{"id":"t","type":"stroke","x":0,"y":0,"width":9,"height":9,"strokeWidth":2,"points":"oops"}]}"#;
    assert!(matches!(Board::parse(text), Err(FormatError::Points { id, .. }) if id == "t"));
}

#[test]
fn fractional_coordinates_are_rounded() {
    let text =
        r#"{"nodes":[{"id":"a","type":"group","x":10.5,"y":-0.4,"width":99.6,"height":1e2}]}"#;
    let node = &Board::parse(text).unwrap().nodes[0];
    assert_eq!((node.x, node.y, node.width, node.height), (11, 0, 100, 100));
}

#[test]
fn a_coordinate_that_does_not_fit_is_an_error() {
    let text = r#"{"nodes":[{"id":"a","type":"group","x":1e30,"y":0,"width":1,"height":1}]}"#;
    assert!(matches!(Board::parse(text), Err(FormatError::Json(_))));
}

#[test]
fn extensions_are_written_only_when_they_differ_from_json_canvas() {
    let written = Board::parse(EXAMPLE).unwrap().write();
    let line = |id: &str| {
        written
            .lines()
            .find(|l| l.contains(&format!("\"id\":\"{id}\"")))
            .unwrap()
    };
    assert!(!line("formato").contains("\"shape\""));
    assert!(line("indice").contains(r#""shape":"ellipse""#));
    assert!(!line("e2").contains("\"route\""));
    assert!(line("e3").contains(r#""route":"straight""#));
    assert!(line("rodeo").contains(r#""type":"stroke""#));
}
