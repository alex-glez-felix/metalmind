//! Consultas de tags: el parseo en `insta` y el resultado sobre un vault en
//! tabla (05 §4, 06 §9).

use std::fmt::Write as _;

use pretty_assertions::assert_eq;
use rstest::rstest;

use super::{key, tags};
use crate::path::VaultPath;
use crate::tags::{QueryWord, TagKey, TagQuery, Tags};

const QUERIES: &[&str] = &[
    "",
    "#rust",
    "#Rust #nota -#archivado",
    "-#archivado",
    "-#",
    "#rust parser",
    "#rust #RUST",
    "#2024/10 #",
    "-nota ##a",
    "#🦀 -#canción",
];

fn keys(keys: &[TagKey]) -> Vec<&str> {
    keys.iter().map(TagKey::as_str).collect()
}

#[test]
fn parse_table() {
    let mut out = String::new();
    for query in QUERIES {
        let parsed = TagQuery::parse(query);
        writeln!(
            out,
            "{query:?}\n    incluye {:?} excluye {:?} sin tags {}\n    palabras {:?} inválidos {:?}",
            keys(&parsed.query.include),
            keys(&parsed.query.exclude),
            parsed.query.untagged,
            parsed.words,
            parsed.invalid,
        )
        .unwrap();
    }
    insta::assert_snapshot!(out);
}

/// Cada palabra con sus bytes, como las marca el finder: los espacios de
/// más no cuentan y `##a` no es `#a`.
#[test]
fn words_with_their_bytes() {
    let text = "  #rust -#Nada ñandú  -# ##a #2024/10 -";
    let words: Vec<(&str, QueryWord)> = TagQuery::words(text)
        .map(|(range, word)| (&text[range], word))
        .collect();
    assert_eq!(
        words,
        [
            ("#rust", QueryWord::Include(key("rust"))),
            ("-#Nada", QueryWord::Exclude(key("nada"))),
            ("ñandú", QueryWord::Text),
            ("-#", QueryWord::Untagged),
            ("##a", QueryWord::Invalid),
            ("#2024/10", QueryWord::Invalid),
            ("-", QueryWord::Text),
        ]
    );
}

fn vault() -> Tags {
    tags(&[
        (
            "tags/rust.md",
            "---\naliases: [rs]\nparents: lenguajes\n---\nDe qué trata #rust.",
        ),
        ("tags/gpui.md", "---\nparents: [rust, ui]\n---\n"),
        ("tags/a.md", "---\nparents: b\n---\n"),
        ("tags/b.md", "---\nparents: a\n---\n"),
        ("rust.md", "#Rust y #nota"),
        ("rs.md", "#rs"),
        ("gpui.md", "---\ntags: [gpui]\n---\n"),
        ("archivada.md", "#gpui #archivado"),
        ("python.md", "#lenguajes"),
        ("ui.md", "#ui #nota"),
        ("ciclo-a.md", "#a"),
        ("ciclo-b.md", "#b"),
        ("buzon.md", "Sin tags, pero dice `#codigo`."),
        ("vacia.md", ""),
    ])
}

#[rstest]
#[case::nothing("", &["archivada.md", "buzon.md", "ciclo-a.md", "ciclo-b.md", "gpui.md", "python.md", "rs.md", "rust.md", "ui.md", "vacia.md"])]
#[case::tag_brings_alias_and_descendants("#rust", &["archivada.md", "gpui.md", "rs.md", "rust.md"])]
#[case::by_alias("#RS", &["archivada.md", "gpui.md", "rs.md", "rust.md"])]
#[case::transitive("#lenguajes", &["archivada.md", "gpui.md", "python.md", "rs.md", "rust.md"])]
#[case::intersection("#rust #nota", &["rust.md"])]
#[case::exclusion("#rust -#archivado", &["gpui.md", "rs.md", "rust.md"])]
#[case::exclusion_alone("-#lenguajes", &["buzon.md", "ciclo-a.md", "ciclo-b.md", "ui.md", "vacia.md"])]
#[case::several_parents("#ui", &["archivada.md", "gpui.md", "ui.md"])]
#[case::cycle_a("#a", &["ciclo-a.md", "ciclo-b.md"])]
#[case::cycle_b("#b", &["ciclo-a.md", "ciclo-b.md"])]
#[case::inbox("-#", &["buzon.md", "vacia.md"])]
#[case::inbox_with_exclusion("-# -#nota", &["buzon.md", "vacia.md"])]
#[case::inbox_with_tag("-# #rust", &[])]
#[case::unknown("#nada", &[])]
#[case::unknown_exclusion("-#nada #nota", &["rust.md", "ui.md"])]
fn query_table(#[case] query: &str, #[case] expected: &[&str]) {
    let tags = vault();
    let parsed = TagQuery::parse(query);
    let found: Vec<&str> = parsed
        .query
        .notes(&tags)
        .into_iter()
        .map(VaultPath::as_str)
        .collect();
    assert_eq!(found, expected);
}

#[test]
fn counts_are_of_distinct_notes() {
    let tags = vault();
    let count = |name: &str| tags.count(&TagKey::new(name));
    assert_eq!(count("rust"), 4);
    assert_eq!(count("lenguajes"), 5);
    assert_eq!(count("nota"), 2);
}
