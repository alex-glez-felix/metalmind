//! Lo que promete el formato (09 §2.3): escribir dos veces da los mismos
//! bytes, las claves desconocidas se conservan y un `.canvas` de Obsidian se
//! reescribe sin cambios de significado.

#![allow(
    clippy::unwrap_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

use metalmind_board::Board;
use pretty_assertions::assert_eq;
use proptest::prelude::*;
use rstest::rstest;
use serde_json::{Map, Value, json};

const FROM_OBSIDIAN: &str = include_str!("fixtures/obsidian.canvas");
const EXAMPLE: &str = include_str!("fixtures/arquitectura.canvas");

fn rewrite(text: &str) -> String {
    Board::parse(text).unwrap().write()
}

fn value(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

#[rstest]
#[case(FROM_OBSIDIAN)]
#[case(EXAMPLE)]
#[case("")]
fn writing_twice_gives_the_same_bytes(#[case] text: &str) {
    let once = rewrite(text);
    assert_eq!(rewrite(&once), once);
}

#[test]
fn a_board_written_canonically_is_rewritten_byte_for_byte() {
    assert_eq!(rewrite(EXAMPLE), EXAMPLE);
}

#[test]
fn a_canvas_from_obsidian_keeps_all_its_meaning() {
    assert_eq!(value(&rewrite(FROM_OBSIDIAN)), value(FROM_OBSIDIAN));
}

/// Lo que difiere solo en la forma: lo que JSON Canvas asume si no se dice.
#[rstest]
#[case(r#"{"id":"e","fromNode":"a","fromEnd":"none","toNode":"a"}"#, json!({"id":"e","fromNode":"a","toNode":"a"}))]
#[case(r#"{"id":"e","fromNode":"a","toNode":"a","toEnd":"arrow"}"#, json!({"id":"e","fromNode":"a","toNode":"a"}))]
#[case(r#"{"id":"e","fromNode":"a","toNode":"a","color":null}"#, json!({"id":"e","fromNode":"a","toNode":"a"}))]
fn explicit_defaults_are_dropped_without_changing_the_meaning(
    #[case] edge: &str,
    #[case] expected: Value,
) {
    let text = format!(
        r#"{{"nodes":[{{"id":"a","type":"group","x":0,"y":0,"width":1,"height":1}}],"edges":[{edge}]}}"#
    );
    assert_eq!(value(&rewrite(&text))["edges"][0], expected);
}

#[test]
fn an_explicit_rectangle_is_the_box_of_json_canvas() {
    let text = r#"{"nodes":[{"id":"a","type":"text","x":0,"y":0,"width":1,"height":1,"shape":"rectangle","text":"t"}]}"#;
    assert!(!rewrite(text).contains("shape"));
}

#[rstest]
#[case::key_of_another_type_on_a_file(
    r#"{"nodes":[{"id":"a","type":"file","file":"n.md","x":0,"y":0,"width":1,"height":1,"text":"x","label":"y","shape":"z"}]}"#,
    &[r#""text":"x","label":"y","shape":"z"}"#],
)]
#[case::key_of_another_type_on_a_group(
    r#"{"nodes":[{"id":"a","type":"group","x":0,"y":0,"width":1,"height":1,"url":"u","label":"l","points":"p"}]}"#,
    &[r#""label":"l","url":"u","points":"p"}"#],
)]
#[case::order_and_nesting(
    r#"{"nodes":[{"zeta":{"b":1,"a":[2,{"d":3,"c":null}]},"id":"a","type":"group","x":0,"y":0,"width":1,"height":1,"alfa":1.50}]}"#,
    &[r#""zeta":{"b":1,"a":[2,{"d":3,"c":null}]},"alfa":1.5}"#],
)]
#[case::on_an_edge(
    r#"{"nodes":[{"id":"a","type":"group","x":0,"y":0,"width":1,"height":1}],"edges":[{"z":1,"id":"e","fromNode":"a","toNode":"a","a":"x\ny"}]}"#,
    &[r#""z":1,"a":"x\ny"}"#],
)]
#[case::next_to_nodes_and_edges(
    r#"{"zeta":{"b":1,"a":2},"nodes":[],"alfa":[1],"edges":[]}"#,
    &["\"zeta\":{\"b\":1,\"a\":2},\n\t\"alfa\":[1]\n}"],
)]
fn unknown_keys_are_kept_in_their_order(#[case] text: &str, #[case] expected: &[&str]) {
    let written = rewrite(text);
    for piece in expected {
        assert!(written.contains(piece), "falta {piece} en {written}");
    }
    assert_eq!(rewrite(&written), written);
}

fn side() -> impl Strategy<Value = Option<&'static str>> {
    proptest::option::of(prop::sample::select(vec!["top", "right", "bottom", "left"]))
}

fn text() -> impl Strategy<Value = String> {
    // Comillas, saltos, multibyte y los separadores que JSON no escapa.
    "[a-z \"\\\\\n\té日本🦀\u{2028}]{0,12}"
}

fn unknown_keys() -> impl Strategy<Value = Vec<(String, Value)>> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::from),
        any::<i64>().prop_map(Value::from),
        (-1e9_f64..1e9).prop_map(Value::from),
        text().prop_map(Value::from),
    ];
    let value = leaf.prop_recursive(2, 8, 3, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..3).prop_map(Value::from),
            prop::collection::vec(("[k-n]{1,3}", inner), 0..3)
                .prop_map(|pairs| Value::Object(pairs.into_iter().collect())),
        ]
    });
    prop::collection::vec(("x-[a-z]{1,6}", value), 0..3)
}

fn with_keys(mut object: Value, unknown: Vec<(String, Value)>) -> Value {
    if let Value::Object(map) = &mut object {
        map.extend(unknown);
    }
    object
}

fn node(index: usize) -> impl Strategy<Value = Value> {
    let id = format!("n{index}");
    let kind = prop_oneof![
        (
            text(),
            prop::sample::select(vec!["rectangle", "ellipse", "diamond", "none"])
        )
            .prop_map(|(text, shape)| json!({"type": "text", "text": text, "shape": shape})),
        (text(), proptest::option::of(text()))
            .prop_map(|(file, sub)| json!({"type": "file", "file": file, "subpath": sub})),
        text().prop_map(|url| json!({"type": "link", "url": url})),
        proptest::option::of(text()).prop_map(|label| json!({"type": "group", "label": label})),
        (
            0.1_f32..50.0,
            prop::collection::vec((any::<i16>(), any::<i16>()), 0..5)
        )
            .prop_map(|(width, points)| {
                let points: Vec<String> = points.iter().map(|(x, y)| format!("{x},{y}")).collect();
                json!({"type": "stroke", "strokeWidth": width, "points": points.join(" ")})
            }),
    ];
    (
        kind,
        any::<(i32, i32, i32, i32)>(),
        proptest::option::of(text()),
        unknown_keys(),
    )
        .prop_map(move |(kind, (x, y, width, height), color, unknown)| {
            let mut object: Map<String, Value> = [("id".to_owned(), Value::from(id.clone()))]
                .into_iter()
                .collect();
            object.extend(
                [("x", x), ("y", y), ("width", width), ("height", height)]
                    .map(|(key, n)| (key.to_owned(), Value::from(n))),
            );
            object.insert("color".to_owned(), color.map_or(Value::Null, Value::from));
            if let Value::Object(kind) = kind {
                object.extend(kind);
            }
            with_keys(Value::Object(object), unknown)
        })
}

fn edge(index: usize, nodes: usize) -> impl Strategy<Value = Value> {
    let id = format!("e{index}");
    (
        (0..nodes, side(), 0..nodes, side()),
        proptest::option::of(prop::sample::select(vec!["none", "arrow"])),
        proptest::option::of(prop::sample::select(vec!["curve", "straight", "elbow"])),
        proptest::option::of(text()),
        unknown_keys(),
    )
        .prop_map(
            move |((from, from_side, to, to_side), tip, route, label, unknown)| {
                with_keys(
                    json!({
                        "id": id, "fromNode": format!("n{from}"), "fromSide": from_side,
                        "toNode": format!("n{to}"), "toSide": to_side, "toEnd": tip,
                        "route": route, "label": label,
                    }),
                    unknown,
                )
            },
        )
}

fn board() -> impl Strategy<Value = String> {
    (1..6_usize).prop_flat_map(|nodes| {
        let node_list: Vec<_> = (0..nodes).map(node).collect();
        let edge_list: Vec<_> = (0..4).map(|i| edge(i, nodes)).collect();
        (node_list, edge_list, unknown_keys()).prop_map(|(nodes, edges, unknown)| {
            with_keys(json!({"nodes": nodes, "edges": edges}), unknown).to_string()
        })
    })
}

proptest! {
    #[test]
    fn an_unknown_float_is_rewritten_exactly(number in prop::num::f64::NORMAL) {
        let text = format!(r#"{{"nodes":[],"edges":[],"x-float":{}}}"#, Value::from(number));
        let once = rewrite(&text);
        prop_assert_eq!(value(&once)["x-float"].as_f64(), Some(number));
        prop_assert_eq!(rewrite(&once), once);
    }

    #[test]
    fn any_board_is_stable_after_the_first_write(text in board()) {
        let once = rewrite(&text);
        prop_assert_eq!(rewrite(&once), once.clone());
        prop_assert_eq!(Board::parse(&once).unwrap(), Board::parse(&text).unwrap());
    }

    #[test]
    fn each_element_takes_one_line(text in board()) {
        let board = Board::parse(&text).unwrap();
        let written = board.write();
        let extra_lines = board.extra.len();
        let framing = 2 + 2 + usize::from(!board.nodes.is_empty()) + usize::from(!board.edges.is_empty());
        prop_assert_eq!(written.lines().count(), framing + board.nodes.len() + board.edges.len() + extra_lines);
    }

    #[test]
    fn unknown_keys_survive_in_any_board(text in board()) {
        let input = value(&text);
        let output = value(&rewrite(&text));
        for section in ["nodes", "edges"] {
            let pairs = input[section].as_array().unwrap().iter().zip(output[section].as_array().unwrap());
            for (before, after) in pairs {
                for (key, kept) in before.as_object().unwrap().iter().filter(|(k, _)| k.starts_with("x-")) {
                    prop_assert_eq!(&after[key], kept);
                }
            }
        }
    }
}
