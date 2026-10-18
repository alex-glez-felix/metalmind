//! La vista de tags del panel con su buzón, el modo focus y el
//! autocompletado de `#` (05 §3, §5; 06 §9).

use std::fmt::Write as _;

use metalmind_syntax::TagPlace;
use pretty_assertions::assert_eq;
use rstest::rstest;

use super::graph::panel;
use super::{Fixture, key, path};
use crate::tags::{Scope, ScopeError, TagColor, TagIcon, TagQuery, Tags};

fn fixture() -> Fixture {
    Fixture::new(&[
        (
            "tags/rust.md",
            "---\ncolor: naranja\nicon: 🦀\naliases: [Rs]\nparents: [lenguajes]\n---\nNotas de #gpui.\n",
        ),
        ("tags/gpui.md", "---\nparents: [rust, ui]\n---\n"),
        ("tags/archivado.md", ""),
        ("p/uno.md", "#gpui y #Rust"),
        ("p/dos.md", "#rs"),
        ("p/tres.md", "Sin tags."),
        ("p/img.png", ""),
        ("otros/Cuatro.md", "#Rust #ui"),
        ("otros/cinco.md", ""),
        ("otros/seis.md", "#2024"),
    ])
}

/// El panel dentro de `scope`, con el buzón al final.
fn render(tags: &Tags, scope: &str) -> String {
    let filter = Scope::parse(scope).unwrap().filter(tags);
    let tree = tags.tree(&filter);
    let mut out = panel(tags, &filter);
    writeln!(out, "Sin tags").unwrap();
    for note in tree.inbox() {
        writeln!(out, "  · {note}").unwrap();
    }
    out
}

#[test]
fn the_panel_without_focus() {
    let tags = fixture().tags();
    assert_eq!(
        render(&tags, ""),
        "\
2024 (1)
  · otros/seis.md
archivado (0)
lenguajes (3) (solo como padre)
  Rust (3)
    gpui (1)
      · p/uno.md
    · otros/Cuatro.md
    · p/dos.md
    · p/uno.md
ui (2)
  gpui (1)
    · p/uno.md
  · otros/Cuatro.md
Sin tags
  · otros/cinco.md
  · p/tres.md
"
    );
    let rust = tags.tree(&Scope::default().filter(&tags));
    let rust = rust.node(&key("rust")).unwrap();
    assert_eq!(rust.name, "Rust");
    assert_eq!(rust.color, Some(TagColor::Named("naranja".into())));
    assert_eq!(rust.icon, Some(TagIcon::Emoji("🦀".into())));
    assert!(rust.has_page);
}

#[test]
fn in_focus_the_panel_counts_inside_the_scope() {
    let tags = fixture().tags();
    assert_eq!(
        render(&tags, "p/"),
        "\
lenguajes (2) (solo como padre)
  Rust (2)
    gpui (1)
      · p/uno.md
    · p/dos.md
    · p/uno.md
ui (1)
  gpui (1)
    · p/uno.md
Sin tags
  · p/tres.md
"
    );
    assert_eq!(
        render(&tags, "#ui"),
        "\
lenguajes (2) (solo como padre)
  Rust (2)
    gpui (1)
      · p/uno.md
    · otros/Cuatro.md
    · p/uno.md
ui (2)
  gpui (1)
    · p/uno.md
  · otros/Cuatro.md
Sin tags
"
    );
    assert_eq!(
        render(&tags, "-#"),
        "Sin tags\n  · otros/cinco.md\n  · p/tres.md\n"
    );
}

#[rstest]
#[case::empty("", None, "")]
#[case::folder("proyectos/", Some("proyectos"), "")]
#[case::folder_without_slash("a/b #rust", Some("a/b"), "#rust")]
#[case::tags_only("#rust -#archivado", None, "#rust -#archivado")]
#[case::inbox("-# p", Some("p"), "-#")]
fn parsing_a_scope(#[case] text: &str, #[case] folder: Option<&str>, #[case] tags: &str) {
    let scope = Scope::parse(text).unwrap();
    assert_eq!(scope.folder, folder.map(path));
    assert_eq!(scope.tags, TagQuery::parse(tags).query);
}

#[rstest]
#[case("a/ b/", ScopeError::TwoFolders("a/".into(), "b/".into()))]
#[case("#a/b", ScopeError::InvalidTag("#a/b".into()))]
#[case("/", ScopeError::InvalidFolder("/".into()))]
#[case("../x", ScopeError::InvalidFolder("../x".into()))]
fn invalid_scopes(#[case] text: &str, #[case] error: ScopeError) {
    assert_eq!(Scope::parse(text), Err(error));
}

#[test]
fn what_a_scope_offers() {
    let tags = fixture().tags();
    let offered = |scope: &str| -> Vec<&str> {
        let filter = Scope::parse(scope).unwrap().filter(&tags);
        let all = [
            "tags/rust.md",
            "p/uno.md",
            "p/dos.md",
            "p/tres.md",
            "p/img.png",
            "otros/cinco.md",
        ];
        all.into_iter()
            .filter(|file| filter.contains(&path(file)))
            .collect()
    };
    assert_eq!(
        offered(""),
        [
            "p/uno.md",
            "p/dos.md",
            "p/tres.md",
            "p/img.png",
            "otros/cinco.md"
        ]
    );
    assert_eq!(
        offered("p/"),
        ["p/uno.md", "p/dos.md", "p/tres.md", "p/img.png"]
    );
    assert_eq!(offered("p #rust"), ["p/uno.md", "p/dos.md"]);
    assert_eq!(offered("-#"), ["p/tres.md", "otros/cinco.md"]);
    assert_eq!(offered("tags/"), ["tags/rust.md"]);
}

#[test]
fn a_new_note_in_focus_gets_the_included_tags() {
    let tags = fixture().tags();
    let scope = Scope::parse("p/ #rs #UI -#archivado").unwrap();
    assert_eq!(scope.new_note_tags(&tags), ["Rust", "ui"]);
}

#[test]
fn completions_by_count_with_aliases() {
    let tags = fixture().tags();
    let found: Vec<(String, Vec<String>, usize)> = tags
        .completions()
        .into_iter()
        .map(|completion| (completion.name, completion.aliases, completion.count))
        .collect();
    let row = |name: &str, aliases: &[&str], count| {
        (
            name.to_owned(),
            aliases.iter().map(|alias| (*alias).to_owned()).collect(),
            count,
        )
    };
    assert_eq!(
        found,
        [
            row("lenguajes", &[], 3),
            row("Rust", &["Rs"], 3),
            row("ui", &[], 2),
            row("2024", &[], 1),
            row("gpui", &[], 1),
            row("archivado", &[], 0),
        ]
    );
    let year = tags
        .completions()
        .into_iter()
        .find(|c| c.name == "2024")
        .unwrap();
    assert_eq!(year.text(TagPlace::Inline), "2024");
    assert_eq!(year.text(TagPlace::Tags), "\"2024\"");
}
