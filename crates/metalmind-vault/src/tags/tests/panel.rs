//! La vista de tags del panel con su buzón, el modo focus y el
//! autocompletado de `#` (05 §3, §5; 06 §9).

use std::collections::BTreeSet;
use std::fmt::Write as _;

use metalmind_syntax::TagPlace;
use pretty_assertions::assert_eq;
use rstest::rstest;

use super::graph::panel;
use super::{Fixture, key, path, scope, tags};
use crate::tags::{Scope, ScopeError, ScopeWord, TagColor, TagIcon, TagKey, TagQuery, Tags};

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
fn render(tags: &Tags, text: &str) -> String {
    let filter = scope(text).unwrap().filter(tags);
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
    let scope = scope(text).unwrap();
    assert_eq!(scope.folder, folder.map(path));
    assert_eq!(scope.tags, TagQuery::parse(tags).query);
}

/// Quien separa las palabras decide qué es carpeta: una con espacios, o
/// una que empieza con `#`.
#[test]
fn a_folder_word_is_a_folder_as_is() {
    let scope = Scope::from_words([ScopeWord::Folder("Mis notas/"), ScopeWord::Tag("#rust")]);
    assert_eq!(scope.unwrap().folder, Some(path("Mis notas")));
    let scope = Scope::from_words([ScopeWord::Folder("#ideas")]);
    assert_eq!(scope.unwrap().folder, Some(path("#ideas")));
}

#[rstest]
#[case("a/ b/", ScopeError::TwoFolders("a/".into(), "b/".into()))]
#[case("#a/b", ScopeError::InvalidTag("#a/b".into()))]
#[case("/", ScopeError::InvalidFolder("/".into()))]
#[case("../x", ScopeError::InvalidFolder("../x".into()))]
fn invalid_scopes(#[case] text: &str, #[case] error: ScopeError) {
    assert_eq!(scope(text), Err(error));
}

#[test]
fn what_a_scope_offers() {
    let tags = fixture().tags();
    let offered = |text: &str| -> Vec<&str> {
        let filter = scope(text).unwrap().filter(&tags);
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
    let scope = scope("p/ #rs #UI -#archivado").unwrap();
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

/// Lo que el vault solo sabe por la versión guardada de `a.md` y la nota de
/// ahora ya no escribe fuera del tag a medio escribir (`[` `]` en `now`):
/// el autoguardado de `#rus` antes de borrar la `s` no deja `rus` en la
/// lista de `#ru`. Lo que nombra otra nota, una página o un `parents` sí es
/// del vault.
#[rstest]
#[case::lo_escrito_antes("hola #rus", "hola #[ru]", &["rus"])]
#[case::lo_escrito_igual("hola #ru", "hola #[ru]", &["ru"])]
#[case::sigue_escrito_en_otro_lado("#rus y #ru", "#rus y #[r]", &["ru"])]
#[case::tambien_en_otra_nota("hola #otra", "hola #[ot]", &[])]
#[case::tiene_pagina("hola #pagina", "hola #[pa]", &[])]
#[case::lo_nombra_un_parents("hola #padre", "hola #[pa]", &[])]
#[case::en_el_frontmatter("---\ntags: [rus]\n---\n", "---\ntags: [[ru]]\n---\n", &["rus"])]
fn unwritten_in_a_note(#[case] saved: &str, #[case] now: &str, #[case] expected: &[&str]) {
    let tags = tags(&[
        ("a.md", saved),
        ("b.md", "#otra"),
        ("tags/hijo.md", "---\nparents: [padre]\n---\n"),
        ("tags/pagina.md", ""),
    ]);
    let start = now.rfind('[').unwrap();
    let end = now.rfind(']').unwrap() - 1;
    let text = format!(
        "{}{}{}",
        &now[..start],
        &now[start + 1..=end],
        &now[end + 2..]
    );
    let unwritten = tags.unwritten_in(&path("a.md"), || text, &(start..end));
    let expected: BTreeSet<TagKey> = expected.iter().map(|name| key(name)).collect();
    assert_eq!(unwritten, expected);
}

/// Si la nota no escribe nada que solo ella nombre, no hace falta leerla.
#[test]
fn unwritten_in_does_not_read_without_tags_of_its_own() {
    let tags = tags(&[("a.md", "#otra"), ("b.md", "#otra")]);
    let unwritten = tags.unwritten_in(&path("a.md"), || unreachable!(), &(1..3));
    assert!(unwritten.is_empty());
}
