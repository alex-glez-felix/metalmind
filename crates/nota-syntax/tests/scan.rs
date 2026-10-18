#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

mod support;

use std::fs;

use nota_syntax::{LinkSyntax, Problem, Subpath, scan};
use pretty_assertions::assert_eq;
use rstest::rstest;

#[test]
fn fixtures() {
    insta::glob!("fixtures/*.md", |path| {
        let text = fs::read_to_string(path).unwrap();
        insta::assert_snapshot!(support::render(&text, &scan(&text)));
    });
}

fn found(text: &str) -> Vec<String> {
    support::found(text, &scan(text))
}

#[rstest]
#[case::inline_code("`[[a]]` [[b]]", &["[[b]]"])]
#[case::inline_code_with_backticks("`` `#a` `` #b", &["#b"])]
#[case::unclosed_inline_code("` [[a]]", &["[[a]]"])]
#[case::backtick_fence("```\n[[a]]\n```\n[[b]]", &["[[b]]"])]
#[case::tilde_fence("~~~\n#a\n~~~\n#b", &["#b"])]
#[case::fence_with_info("```rust\n#a\n```\n#b", &["#b"])]
#[case::unclosed_fence("```\n[[a]]\n#b", &[])]
#[case::inline_comment("%%[[a]]%% [[b]]", &["[[b]]"])]
#[case::block_comment("%%\n[[a]]\n%%\n#b", &["#b"])]
#[case::unclosed_comment("[[a]] %% [[b]]", &["[[a]]"])]
#[case::comment_inside_code("`%%` [[a]] `%%`", &["[[a]]"])]
#[case::code_inside_comment("%% ` %% [[a]] `", &["[[a]]"])]
#[case::escaped_comment("\\%%[[a]]", &["[[a]]"])]
fn code_and_comments_hide_syntax(#[case] text: &str, #[case] expected: &[&str]) {
    assert_eq!(found(text), expected);
}

#[rstest]
#[case::url_fragment("https://example.com/#a #b", &["#b"])]
#[case::markdown_url("[x](https://example.com/#a) #b", &["#b"])]
#[case::autolink("<https://example.com/#a>", &[])]
#[case::glued_to_text("texto#a", &[])]
#[case::after_paren("(#a)", &[])]
#[case::only_digits("#1984 #y1984 #2024/10", &["#y1984", "#2024/10"])]
#[case::heading("# Heading", &[])]
#[case::heading_with_tag("## Heading #a", &["#a"])]
#[case::no_space_is_a_tag("#Heading", &["#Heading"])]
#[case::ends_at_punctuation("#a. #b, #c! #d? #e:", &["#a", "#b", "#c", "#d", "#e"])]
#[case::ends_at_hash("#a#b", &["#a"])]
#[case::nested("#a/b/c", &["#a/b/c"])]
#[case::escaped("\\#a", &[])]
#[case::inside_wikilink("[[nota#a]]", &["[[nota#a]]"])]
fn tags(#[case] text: &str, #[case] expected: &[&str]) {
    assert_eq!(found(text), expected);
}

#[rstest]
#[case::accents("ñ [[canción]] #über", &["[[canción]]", "#über"])]
#[case::emoji_tag("#🔥! #a🎉b", &["#🔥", "#a🎉b"])]
#[case::zwj_sequence("#👨‍👩‍👧 fin", &["#👨‍👩‍👧"])]
#[case::after_multibyte_letter("é#a", &[])]
#[case::cjk_punctuation("#日本語。", &["#日本語"])]
#[case::ellipsis("#tag…", &["#tag"])]
#[case::arabic_comma("#x، #y", &["#x", "#y"])]
#[case::letters_in_punctuation_blocks("#xª #x々", &["#xª", "#x々"])]
#[case::subdivision_flag("#🏴\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F}.", &["#🏴\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F}"])]
#[case::emoji_link("[[🇲🇽|bandera]]", &["[[🇲🇽|bandera]]"])]
fn multibyte(#[case] text: &str, #[case] expected: &[&str]) {
    assert_eq!(found(text), expected);
}

#[rstest]
#[case::link_and_tag("[[a]]\r\n#b\r\n", &["[[a]]", "#b"])]
#[case::fence("```\r\n[[a]]\r\n```\r\n[[b]]", &["[[b]]"])]
#[case::no_line_break_inside("[[a\r\nb]]", &[])]
#[case::frontmatter("---\r\ntags: [x]\r\n---\r\n#y", &["#y"])]
#[case::code_span_across_lines("`a\r\n[[b]]` [[c]]", &["[[c]]"])]
fn crlf(#[case] text: &str, #[case] expected: &[&str]) {
    assert_eq!(found(text), expected);
}

#[rstest]
#[case::empty("[[]]", &[])]
#[case::only_alias("[[|a]]", &[])]
#[case::line_break("[[a\nb]]", &[])]
#[case::nested_open("[[a [[b]]", &["[[b]]"])]
#[case::escaped("\\[[a]]", &[])]
#[case::markdown_local("[x](nota.md) [y](http://x.com)", &["[x](nota.md)"])]
#[case::markdown_embed("![x](a.png)", &["![x](a.png)"])]
#[case::markdown_same_note("[x](#Heading)", &["[x](#Heading)"])]
#[case::markdown_parens("[x](a(b).md)", &["[x](a(b).md)"])]
#[case::markdown_space_before_paren("[x] (nota.md)", &[])]
#[case::markdown_empty("[x]()", &[])]
#[case::markdown_only_hash("[x](#) #a", &["#a"])]
#[case::markdown_title_unclosed("[x](a.md \"t)", &[])]
#[case::markdown_paren_title("[x](a.md (un título)) #b", &["[x](a.md (un título))", "#b"])]
#[case::markdown_escaped_quote_in_title("[x](a.md \"un \\\" t\")", &["[x](a.md \"un \\\" t\")"])]
#[case::markdown_code_in_label("[a `[b` #c](nota.md)", &["[a `[b` #c](nota.md)"])]
#[case::markdown_code_with_bracket_in_label("[a `]` b](nota.md)", &["[a `]` b](nota.md)"])]
#[case::markdown_comment_in_label("[a %%]%% b](nota.md)", &["[a %%]%% b](nota.md)"])]
#[case::markdown_nested_label("[a [b] c](nota.md)", &["[a [b] c](nota.md)"])]
fn links(#[case] text: &str, #[case] expected: &[&str]) {
    assert_eq!(found(text), expected);
}

/// Destino, subruta y alias de un link, como texto.
fn parts(text: &str) -> (String, Option<String>, Option<String>, bool) {
    let scan = scan(text);
    let [link] = scan.links.as_slice() else {
        panic!("se esperaba un link en {text:?}: {:?}", scan.links);
    };
    let subpath = link.subpath.as_ref().map(|subpath| match subpath {
        Subpath::Heading(range) => format!("#{}", &text[range.clone()]),
        Subpath::Block(range) => format!("^{}", &text[range.clone()]),
    });
    let alias = link.alias.clone().map(|range| text[range].to_owned());
    (
        link.text(text, link.target.clone()).into_owned(),
        subpath,
        alias,
        link.embed,
    )
}

#[rstest]
#[case("[[nota]]", "nota", None, None, false)]
#[case("[[nota|alias]]", "nota", None, Some("alias"), false)]
#[case("[[nota#Heading]]", "nota", Some("#Heading"), None, false)]
#[case("[[nota#^bloque]]", "nota", Some("^bloque"), None, false)]
#[case("[[#Heading]]", "", Some("#Heading"), None, false)]
#[case("[[#^bloque|ver]]", "", Some("^bloque"), Some("ver"), false)]
#[case("[[nota#]]", "nota", None, None, false)]
#[case(
    "[[ nota # Heading | alias ]]",
    "nota",
    Some("#Heading"),
    Some("alias"),
    false
)]
#[case("[[nota\\|alias]]", "nota", None, Some("alias"), false)]
#[case("[[a|b|c]]", "a", None, Some("b|c"), false)]
#[case("![[img.png]]", "img.png", None, None, true)]
#[case("![[img.png|300]]", "img.png", None, Some("300"), true)]
#[case("![[nota#sección]]", "nota", Some("#sección"), None, true)]
#[case(
    "[texto](Mi%20Nota.md#Un%20H)",
    "Mi Nota.md",
    Some("#Un%20H"),
    Some("texto"),
    false
)]
#[case("[a](<con espacios.md>)", "con espacios.md", None, Some("a"), false)]
#[case("[a](nota.md \"título\")", "nota.md", None, Some("a"), false)]
#[case("![](img.png)", "img.png", None, None, true)]
fn link_parts(
    #[case] text: &str,
    #[case] target: &str,
    #[case] subpath: Option<&str>,
    #[case] alias: Option<&str>,
    #[case] embed: bool,
) {
    let expected = (
        target.to_owned(),
        subpath.map(str::to_owned),
        alias.map(str::to_owned),
        embed,
    );
    assert_eq!(parts(text), expected);
}

#[test]
fn link_syntax_is_reported() {
    let scan = scan("[[a]] [b](b.md)");
    let syntaxes: Vec<_> = scan.links.iter().map(|link| link.syntax).collect();
    assert_eq!(syntaxes, [LinkSyntax::Wiki, LinkSyntax::Markdown]);
}

#[test]
fn frontmatter_with_crlf() {
    let text = "---\r\ntags: [a, b]\r\naliases: x\r\n---\r\ncuerpo";
    let frontmatter = scan(text).frontmatter.unwrap();
    assert_eq!(frontmatter.tags, ["a", "b"]);
    assert_eq!(frontmatter.aliases, ["x"]);
    assert_eq!(
        &text[frontmatter.span],
        "---\r\ntags: [a, b]\r\naliases: x\r\n---\r"
    );
}

#[test]
fn frontmatter_after_bom() {
    let text = "\u{FEFF}---\ntags: a\n---\n#b";
    let scan = scan(text);
    assert_eq!(scan.frontmatter.unwrap().tags, ["a"]);
    assert_eq!(found(text), ["#b"]);
}

#[rstest]
#[case::fence("\u{FEFF}```\n[[a]]\n```\n[[b]]", &["[[b]]"])]
#[case::tag("\u{FEFF}#tag", &["#tag"])]
fn bom_without_frontmatter(#[case] text: &str, #[case] expected: &[&str]) {
    assert_eq!(found(text), expected);
}

#[test]
fn frontmatter_must_start_the_note() {
    assert_eq!(scan("\n---\ntags: a\n---\n").frontmatter, None);
}

#[test]
fn empty_frontmatter() {
    let frontmatter = scan("---\n---\n#a").frontmatter.unwrap();
    assert_eq!(frontmatter.span, 0..7);
    assert_eq!(frontmatter.yaml, 4..4);
}

#[rstest]
#[case::not_a_mapping("---\n- a\n- b\n---\n", "el frontmatter no es un mapa de claves")]
#[case::field_of_another_type(
    "---\ntags: {a: 1}\n---\n",
    "el campo `tags` del frontmatter no es ni lista ni texto"
)]
fn recoverable_frontmatter_problems(#[case] text: &str, #[case] message: &str) {
    let problems: Vec<String> = scan(text).problems.iter().map(Problem::to_string).collect();
    assert_eq!(problems, [message]);
}

#[test]
fn deeply_nested_yaml_does_not_overflow() {
    let text = format!("---\ntags: {}\n---\n#a", "[".repeat(100_000));
    let scan = scan(&text);
    assert_eq!(scan.problems.len(), 1);
    assert_eq!(found(&text), ["#a"]);
}

/// nota lee solo los nombres en plural; los singulares de antes de Obsidian
/// 1.4 son claves cualquiera.
#[test]
fn singular_field_names_are_not_read() {
    let frontmatter = scan("---\ntag: a\nalias: b\ncssclass: c\n---\n")
        .frontmatter
        .unwrap();
    assert_eq!(
        (
            frontmatter.tags,
            frontmatter.aliases,
            frontmatter.cssclasses
        ),
        (Vec::new(), Vec::new(), Vec::new())
    );
}

/// Rachas sin pareja de largos 1, 2, …, k en un párrafo: buscar el cierre de
/// cada una recorriendo el resto sería cuadrático (millones de bytes por
/// racha); con el índice por largo son milisegundos.
#[test]
fn unclosed_backtick_runs_of_many_lengths_stay_linear() {
    let text: String = (1..=2000).map(|len| "`".repeat(len) + " ").collect();
    let start = std::time::Instant::now();
    let scan = scan(&format!("{text}[[a]]"));
    assert_eq!(scan.links.len(), 1);
    assert!(
        start.elapsed() < std::time::Duration::from_secs(5),
        "{:?}",
        start.elapsed()
    );
}
