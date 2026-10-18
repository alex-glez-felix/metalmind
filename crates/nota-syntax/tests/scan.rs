#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

mod support;

use std::borrow::Cow;
use std::fs;

use nota_syntax::{LinkSyntax, Problem, Resolution, Subpath, TagPlace, resolve, scan};
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
#[case::fence_in_list_item("- ```\n  [[a]]\n  ```\n[[b]]", &["[[b]]"])]
#[case::fence_in_ordered_item("1. ~~~\n   #a\n   ~~~\n#b", &["#b"])]
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
#[case::only_digits("#2024 #y1984 #2024/10", &["#2024", "#y1984", "#2024"])]
#[case::digits_at_end_of_line("primero\nel #1\nfin", &["#1"])]
#[case::digits_end_at_punctuation("año #2024.", &["#2024"])]
#[case::digits_in_url("https://example.com/#123 [x](https://example.com/#123)", &[])]
#[case::digits_in_inline_code("`#123` #124", &["#124"])]
#[case::heading("# Heading", &[])]
#[case::heading_with_tag("## Heading #a", &["#a"])]
#[case::no_space_is_a_tag("#Heading", &["#Heading"])]
#[case::ends_at_punctuation("#a. #b, #c! #d? #e:", &["#a", "#b", "#c", "#d", "#e"])]
#[case::ends_at_hash("#a#b", &["#a"])]
#[case::slash_ends_tag("#a/b/c", &["#a"])]
#[case::trailing_slash("#proyecto/ fin", &["#proyecto"])]
#[case::leading_slash("#/a", &[])]
#[case::only_separators("#- #/ #_ #a//b", &["#a"])]
#[case::file_name_chars("#a\\b #c:d #e*f #g?h #i\"j #k<l #m>n #o|p", &["#a", "#c", "#e", "#g", "#i", "#k", "#m", "#o"])]
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
#[case::markdown_paren_in_paren_title("[x](a.md (un ( título)) #b", &["#b"])]
#[case::markdown_escaped_paren_in_paren_title("[x](a.md (un \\( t))", &["[x](a.md (un \\( t))"])]
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
    let subpath = link
        .subpath_text(text)
        .map(|subpath_text| match link.subpath {
            Some(Subpath::Block(_)) => format!("^{subpath_text}"),
            _ => format!("#{subpath_text}"),
        });
    (
        link.target_text(text).into_owned(),
        subpath,
        link.alias_text(text).map(Cow::into_owned),
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
#[case("[[nota|ver\\*]]", "nota", None, Some("ver*"), false)]
#[case("![[img.png]]", "img.png", None, None, true)]
#[case("![[img.png|300]]", "img.png", None, Some("300"), true)]
#[case("![[nota#sección]]", "nota", Some("#sección"), None, true)]
#[case(
    "[texto](Mi%20Nota.md#Un%20H)",
    "Mi Nota.md",
    Some("#Un H"),
    Some("texto"),
    false
)]
#[case("[a](<con espacios.md>)", "con espacios.md", None, Some("a"), false)]
#[case("[a](nota.md \"título\")", "nota.md", None, Some("a"), false)]
#[case("![](img.png)", "img.png", None, None, true)]
#[case("[x](a\\).md)", "a).md", None, Some("x"), false)]
#[case("[x](<a\\>b.md>)", "a>b.md", None, Some("x"), false)]
#[case("[x](<a \\<b.md>)", "a <b.md", None, Some("x"), false)]
#[case(
    "[50%25 \\*off\\*](nota.md)",
    "nota.md",
    None,
    Some("50%25 *off*"),
    false
)]
#[case("[x](nota\\#1.md#H)", "nota#1.md", Some("#H"), Some("x"), false)]
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
    assert_eq!(support::texts(&frontmatter.tags), ["a", "b"]);
    assert_eq!(support::texts(&frontmatter.aliases), ["x"]);
    assert_eq!(
        &text[frontmatter.span],
        "---\r\ntags: [a, b]\r\naliases: x\r\n---\r"
    );
}

#[test]
fn frontmatter_after_bom() {
    let text = "\u{FEFF}---\ntags: a\n---\n#b";
    let scan = scan(text);
    assert_eq!(support::texts(&scan.frontmatter.unwrap().tags), ["a"]);
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
#[case::text_with_commas(
    "---\ntags: rust, nota\n---\n",
    "`rust, nota` en `tags` del frontmatter no es un tag válido"
)]
#[case::slash(
    "---\ntags: [2024/10]\n---\n",
    "`2024/10` en `tags` del frontmatter no es un tag válido"
)]
#[case::double_hash(
    "---\ntags: \"##project\"\n---\n",
    "`##project` en `tags` del frontmatter no es un tag válido"
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
fn a_text_field_is_one_value() {
    let frontmatter = scan("---\ntags: \"#rust\"\naliases: Uno, dos\ncssclasses: ancho\n---\n")
        .frontmatter
        .unwrap();
    assert_eq!(support::texts(&frontmatter.tags), ["rust"]);
    assert_eq!(support::texts(&frontmatter.aliases), ["Uno, dos"]);
    assert_eq!(support::texts(&frontmatter.cssclasses), ["ancho"]);
}

#[test]
fn a_number_in_frontmatter_tags_is_a_tag() {
    let frontmatter = scan("---\ntags: [a, 2024]\n---\n").frontmatter.unwrap();
    assert_eq!(support::texts(&frontmatter.tags), ["a", "2024"]);
}

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

/// Una línea de `[a](b (` sin cerrar: si el título de cada intento buscara
/// su `)` hasta el fin de la línea, el scan sería cuadrático.
#[test]
fn unclosed_paren_titles_stay_linear() {
    let text = "[a](b (".repeat(100_000);
    let start = std::time::Instant::now();
    assert_eq!(scan(&text).links, []);
    assert!(
        start.elapsed() < std::time::Duration::from_secs(5),
        "{:?}",
        start.elapsed()
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

#[rstest]
#[case("[x](a\\).md)", "a).md")]
#[case("[x](nota\\#1.md#H)", "carpeta/nota#1.md")]
#[case("[x](Mi%20Nota.md)", "Mi Nota.md")]
fn escaped_markdown_destinations_resolve(#[case] text: &str, #[case] path: &str) {
    let scan = scan(text);
    let target = scan.links[0].target_text(text);
    assert_eq!(resolve(&target, [path]), Resolution::Resolved(path));
}

#[test]
fn link_at_finds_the_link_under_the_cursor() {
    let text = "ver [[a]] y `[[b]]` y [c](c.md)";
    let target =
        |offset| nota_syntax::link_at(text, offset).map(|link| text[link.target].to_owned());
    assert_eq!(target(3), None);
    assert_eq!(target(4).as_deref(), Some("a"));
    assert_eq!(target(8).as_deref(), Some("a"));
    assert_eq!(target(9), None);
    assert_eq!(target(14), None);
    assert_eq!(target(22).as_deref(), Some("c.md"));
}

/// La línea del heading que nombra cada subruta.
#[rstest]
#[case::by_name("# Uno\n## Dos", "dos", Some(1))]
#[case::nested("# A\n## X\n# B\n## X", "B#X", Some(3))]
#[case::nested_stays_in_section("# A\n# B\n## X", "A#X", None)]
#[case::first_of_repeated("## X\n## X", "X", Some(0))]
#[case::missing("# Uno", "Dos", None)]
#[case::in_code("```\n# Uno\n```", "Uno", None)]
fn headings_by_subpath(#[case] text: &str, #[case] path: &str, #[case] line: Option<usize>) {
    let found = scan(text)
        .heading(text, path)
        .map(|heading| text[..heading.span.start].matches('\n').count());
    assert_eq!(found, line);
}

#[test]
fn blocks_by_id() {
    let text = "a ^Uno\nb ^dos";
    let scan = scan(text);
    assert_eq!(
        scan.block(text, "uno").map(|block| block.span.clone()),
        Some(2..6)
    );
    assert_eq!(scan.block(text, "tres"), None);
}

#[test]
fn a_tag_longer_than_a_file_name_is_not_a_tag() {
    let longest = format!("#{} fin", "a".repeat(250));
    assert_eq!(found(&longest), [&longest[..251]]);
    assert_eq!(
        found(&format!("#{} fin", "a".repeat(251))),
        Vec::<String>::new()
    );
}

/// Lo que se borra para sacar un valor de su lista deja el YAML válido y
/// sin el valor, se quite el primero, uno del medio o el último.
#[rstest]
#[case::flow_first("---\ntags: [a, b, c]\n---\n", 0, "tags: [b, c]\n")]
#[case::flow_middle("---\ntags: [a, b, c]\n---\n", 1, "tags: [a, c]\n")]
#[case::flow_last("---\ntags: [a, b, c]\n---\n", 2, "tags: [a, b]\n")]
#[case::flow_alone("---\ntags: [a]\n---\n", 0, "tags: []\n")]
#[case::block("---\ntags:\n  - a\n  - b\n---\n", 1, "tags:\n  - a\n")]
#[case::block_crlf("---\r\ntags:\r\n  - a\r\n  - b\r\n---\r\n", 0, "tags:\r\n  - b\r\n")]
fn removing_a_frontmatter_value(#[case] text: &str, #[case] which: usize, #[case] yaml: &str) {
    let frontmatter = scan(text).frontmatter.unwrap();
    let removal = frontmatter.tags[which].removal.clone();
    let rest = format!("{}{}", &text[..removal.start], &text[removal.end..]);
    let after = scan(&rest).frontmatter.unwrap();
    assert_eq!(&rest[after.yaml], yaml);
}

#[rstest]
#[case::plain("---\ntags: rust\n---\n", "rust")]
#[case::hash("---\ntags: \"#rust\"\n---\n", "rust")]
#[case::single_quotes("---\ntags: ['rust']\n---\n", "rust")]
#[case::spaces("---\ntags: [\" rust \"]\n---\n", "rust")]
fn the_literal_range_is_the_tag_name(#[case] text: &str, #[case] name: &str) {
    let frontmatter = scan(text).frontmatter.unwrap();
    let literal = frontmatter.tags[0].literal.clone().unwrap();
    assert_eq!(&text[literal], name);
}

#[rstest]
#[case::escape("---\ntags: [\"r\\u0075st\"]\n---\n")]
#[case::doubled_quote("---\ntags: ['it''s']\n---\n")]
#[case::folded("---\ntags:\n  - a\n    b\n---\n")]
fn a_value_written_differently_is_not_literal(#[case] text: &str) {
    let scan = scan(text);
    let values = scan
        .frontmatter
        .map(|f| f.aliases.into_iter().chain(f.tags).collect::<Vec<_>>());
    assert!(
        values.iter().flatten().all(|value| value.literal.is_none()),
        "{values:?}"
    );
}

#[rstest]
#[case::list(
    "---\ncolor: [rojo]\n---\n",
    "el campo `color` del frontmatter no es un texto"
)]
#[case::mapping(
    "---\nicon: {a: b}\n---\n",
    "el campo `icon` del frontmatter no es un texto"
)]
fn single_fields_are_text(#[case] text: &str, #[case] message: &str) {
    let problems: Vec<String> = scan(text).problems.iter().map(Problem::to_string).collect();
    assert_eq!(problems, [message]);
}

#[test]
fn a_color_written_as_a_comment_is_no_color() {
    let scan = scan("---\ncolor: #d97706\n---\n");
    assert_eq!(
        (scan.frontmatter.unwrap().color, scan.problems),
        (None, Vec::new())
    );
}

/// Cada clave del frontmatter con el texto que ocupa.
fn entries(text: &str) -> Option<Vec<(String, &str)>> {
    let frontmatter = scan(text).frontmatter?;
    let entries = frontmatter.entries?;
    Some(
        entries
            .into_iter()
            .map(|entry| (entry.key, &text[entry.span]))
            .collect(),
    )
}

#[rstest]
#[case::flat("---\na: 1\nb: [x, y]\n---\n", &[("a", "a: 1\n"), ("b", "b: [x, y]\n")])]
#[case::block_list("---\ntags:\n  - a\n  - b\ncolor: rojo\n---\n", &[("tags", "tags:\n  - a\n  - b\n"), ("color", "color: rojo\n")])]
#[case::list_at_column_zero("---\ntags:\n- a\n- b\nicon: x\n---\n", &[("tags", "tags:\n- a\n- b\n"), ("icon", "icon: x\n")])]
#[case::blank_and_comments_stay_out("---\na: 1\n\n# sobre b\nb: 2\n# fin\n---\n", &[("a", "a: 1\n"), ("b", "b: 2\n")])]
#[case::indented_comment_is_inside("---\na:\n  - x\n  # nota\nb: 2\n---\n", &[("a", "a:\n  - x\n  # nota\n"), ("b", "b: 2\n")])]
#[case::multiline_scalar("---\nd: |\n  uno\n  dos\ne: 1\n---\n", &[("d", "d: |\n  uno\n  dos\n"), ("e", "e: 1\n")])]
#[case::quoted_key("---\n\"color\": rojo\n---\n", &[("color", "\"color\": rojo\n")])]
#[case::crlf("---\r\na: 1\r\nb: 2\r\n---\r\n", &[("a", "a: 1\r\n"), ("b", "b: 2\r\n")])]
#[case::empty("---\n---\n", &[])]
#[case::only_comment("---\n# nada\n---\n", &[])]
fn frontmatter_entries(#[case] text: &str, #[case] expected: &[(&str, &str)]) {
    let expected: Vec<(String, &str)> = expected
        .iter()
        .map(|&(key, source)| (key.to_owned(), source))
        .collect();
    assert_eq!(entries(text), Some(expected));
}

#[rstest]
#[case::flow_mapping("---\n{a: 1, b: 2}\n---\n")]
#[case::complex_key("---\n? [a]\n: 1\n---\n")]
#[case::broken("---\ntags: [a\n---\n")]
#[case::not_a_mapping("---\n- a\n---\n")]
fn frontmatter_that_cannot_be_edited(#[case] text: &str) {
    assert_eq!(entries(text), None);
}

/// El tag bajo cada `|` de `marked`, quitando la marca.
fn tag_at(marked: &str) -> Option<(String, TagPlace)> {
    let at = marked.find('|').unwrap();
    let text = marked.replacen('|', "", 1);
    let spot = scan(&text).tag_at(at)?;
    assert!(spot.replace.start <= spot.name.start && spot.name.end <= spot.replace.end);
    Some((text[spot.name].to_owned(), spot.place))
}

#[test]
fn completion_replaces_the_whole_yaml_scalar() {
    let text = "---
tags: [\"#ru\", ok]
---
#gp";
    let scan = scan(text);
    let yaml = scan.tag_at(text.find("ru").unwrap() + 2).unwrap();
    assert_eq!((&text[yaml.name], &text[yaml.replace]), ("ru", "\"#ru\""));
    let inline = scan.tag_at(text.len()).unwrap();
    assert_eq!(
        (&text[inline.name.clone()], &text[inline.replace]),
        ("gp", "gp")
    );
}

#[rstest]
#[case::on_the_hash("hoy |#rust y", Some(("rust", TagPlace::Inline)))]
#[case::inside("hoy #ru|st y", Some(("rust", TagPlace::Inline)))]
#[case::just_after("hoy #rust| y", Some(("rust", TagPlace::Inline)))]
#[case::while_typing("hoy #r|", Some(("r", TagPlace::Inline)))]
#[case::after_the_space("hoy #rust |y", None)]
#[case::only_hash("hoy #|", None)]
#[case::heading("#| título", None)]
#[case::in_code("`#ru|st`", None)]
#[case::in_fence("```\n#ru|st\n```", None)]
#[case::tags_field("---\ntags: [a, ru|st]\n---\n", Some(("rust", TagPlace::Tags)))]
#[case::tags_with_hash("---\ntags: [\"#ru|st\"]\n---\n", Some(("rust", TagPlace::Tags)))]
#[case::block_list("---\ntags:\n  - |rust\n---\n", Some(("rust", TagPlace::Tags)))]
#[case::parents("---\nparents: [lenguajes, ui|]\n---\n", Some(("ui", TagPlace::Parents)))]
#[case::aliases("---\naliases: r|s\n---\n", Some(("rs", TagPlace::Aliases)))]
#[case::other_key("---\ntitulo: ru|st\n---\n", None)]
#[case::invalid_value("---\nparents: [a/|b]\n---\n", None)]
#[case::key_name("---\nta|gs: [a]\n---\n", None)]
#[case::body_after_frontmatter("---\ntags: [a]\n---\n#b|", Some(("b", TagPlace::Inline)))]
fn the_tag_under_a_byte(#[case] marked: &str, #[case] expected: Option<(&str, TagPlace)>) {
    let expected = expected.map(|(name, place)| (name.to_owned(), place));
    assert_eq!(tag_at(marked), expected);
}
