use std::fmt::Write as _;

use pretty_assertions::assert_eq;
use proptest::prelude::*;

use super::{Conceal, Decorations, LineKind, LineStyle, Lines, MathPlace, Role, Styles, TextLines};
use crate::scan;

fn styled(text: &str) -> Vec<LineStyle> {
    let lines = TextLines::new(text);
    let mut styles = Styles::new(lines.line_count());
    styles.ensure(usize::MAX, &lines);
    (0..lines.line_count())
        .map(|line| styles.line(line).unwrap().clone())
        .collect()
}

/// Cada línea con sus tramos: `[texto]rol+atributos~oculto`.
fn render(text: &str) -> String {
    let mut out = String::new();
    for (line, style) in text.split('\n').zip(styled(text)) {
        write!(out, "{:?}", style.kind).unwrap();
        if style.quote > 0 {
            write!(out, " >{}", style.quote).unwrap();
        }
        out.push_str(" |");
        for span in &style.spans {
            write!(out, " [{}]{:?}", &line[span.range.clone()], span.style.role).unwrap();
            for (decoration, flag) in [
                (Decorations::STRONG, "+strong"),
                (Decorations::EMPHASIS, "+em"),
                (Decorations::STRIKE, "+strike"),
                (Decorations::MARK, "+mark"),
                (Decorations::CODE, "+code"),
            ] {
                if span.style.decorations.contains(decoration) {
                    out.push_str(flag);
                }
            }
            match span.conceal {
                Some(Conceal::Hide) => out.push('~'),
                Some(conceal) => write!(out, "~{conceal:?}").unwrap(),
                None => {}
            }
        }
        for link in &style.links {
            write!(out, " ->{:?}", link.target).unwrap();
        }
        for math in &style.math {
            let kind = if math.display { "$$" } else { "$" };
            write!(out, " {kind}{:?}@{:?}", math.latex, math.range).unwrap();
            match math.place {
                MathPlace::Inline => {}
                MathPlace::Alone => out.push_str(" solo"),
                MathPlace::Below { above } => write!(out, "^{above}").unwrap(),
            }
        }
        out.push('\n');
    }
    out
}

const NOTE: &str = include_str!("../../tests/fixtures/estilos.md");

#[test]
fn every_kind_of_syntax() {
    insta::assert_snapshot!(render(NOTE));
}

const MATH: &str = include_str!("../../tests/fixtures/formulas.md");

#[test]
fn formulas() {
    insta::assert_snapshot!(render(MATH));
}

#[test]
fn a_block_follows_its_lines_when_edited() {
    let mut text = String::from("a\n$$\nx\n$$\nb");
    let mut lines = TextLines::new(&text);
    let mut styles = Styles::new(lines.line_count());
    styles.ensure(usize::MAX, &lines);
    assert_eq!(styles.line(3).unwrap().math[0].latex, "\nx\n");
    text = String::from("a\n$$\nx\ny\n$$\nb");
    lines = TextLines::new(&text);
    styles.edit(3, 0, 1);
    styles.ensure(usize::MAX, &lines);
    let closing = &styles.line(4).unwrap().math[0];
    assert_eq!(
        (closing.latex.as_str(), closing.place),
        ("\nx\ny\n", MathPlace::Below { above: 3 })
    );
    assert_eq!(
        (0..lines.line_count())
            .map(|line| styles.line(line).unwrap().clone())
            .collect::<Vec<_>>(),
        styled(&text)
    );
}

#[test]
fn a_line_knows_its_math_block() {
    let text = "a\nx $$ b\nc\n\nd $$ e\nf\n$$ sin cerrar\ng";
    let lines = TextLines::new(text);
    let mut styles = Styles::new(lines.line_count());
    styles.ensure(usize::MAX, &lines);
    let blocks: Vec<_> = (0..lines.line_count())
        .map(|line| styles.math_block(line))
        .collect();
    assert_eq!(
        blocks,
        [
            None,
            Some(1..=4),
            Some(1..=4),
            Some(1..=4),
            Some(1..=4),
            None,
            Some(6..=7),
            Some(6..=7)
        ]
    );
}

#[test]
fn a_frontmatter_closed_far_below_restyles_what_it_covers() {
    let mut text = String::from("---\ntags: [a]\n#b\n\n[[c]]\n");
    let mut lines = TextLines::new(&text);
    let mut styles = Styles::new(lines.line_count());
    styles.ensure(usize::MAX, &lines);
    assert_eq!(styles.line(1).unwrap().kind, LineKind::Text);
    text.push_str("---");
    lines = TextLines::new(&text);
    styles.edit(5, 1, 1);
    styles.ensure(usize::MAX, &lines);
    assert_eq!(styles.line(4).unwrap().kind, LineKind::Frontmatter);
    assert_eq!(
        (0..lines.line_count())
            .map(|line| styles.line(line).unwrap().clone())
            .collect::<Vec<_>>(),
        styled(&text)
    );
}

#[test]
fn only_what_is_asked_for_is_styled() {
    let text = "a\n\nb\n\nc\n\nd";
    let lines = TextLines::new(text);
    let mut styles = Styles::new(lines.line_count());
    styles.ensure(2, &lines);
    assert!(styles.line(2).is_some());
    assert!(styles.styled() < lines.line_count());
}

/// Trozos que arman markdown a medias, como los de `tests/proptest.rs`
/// más los de bloque y negrita.
fn piece() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("[[".to_owned()),
        Just("]]".to_owned()),
        Just("![[".to_owned()),
        Just("[".to_owned()),
        Just("](".to_owned()),
        Just(")".to_owned()),
        Just("|".to_owned()),
        Just("\\".to_owned()),
        Just("#".to_owned()),
        Just("# ".to_owned()),
        Just("#^".to_owned()),
        Just("`".to_owned()),
        Just("```".to_owned()),
        Just("~~~".to_owned()),
        Just("%%".to_owned()),
        Just("---".to_owned()),
        Just("- ".to_owned()),
        Just("- [x] ".to_owned()),
        Just("> ".to_owned()),
        Just("*".to_owned()),
        Just("**".to_owned()),
        Just("_".to_owned()),
        Just("~~".to_owned()),
        Just("==".to_owned()),
        Just("$".to_owned()),
        Just("$$".to_owned()),
        Just("\\$".to_owned()),
        Just("5".to_owned()),
        Just("\n".to_owned()),
        Just("\n\n".to_owned()),
        Just(" ".to_owned()),
        Just("é".to_owned()),
        Just("\u{FEFF}".to_owned()),
        "[a-z0-9/.]{1,4}",
    ]
}

fn note() -> impl Strategy<Value = String> {
    prop::collection::vec(piece(), 0..40).prop_map(|pieces| pieces.concat())
}

fn line_starts(text: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(text.match_indices('\n').map(|(i, _)| i + 1))
        .collect()
}

/// Lo que el editor y el índice tienen que ver igual, en bytes de la nota:
/// links (con su destino), tags, headings (línea y nivel) y bloques.
#[derive(Debug, Default, PartialEq, Eq)]
struct Found {
    links: Vec<(usize, usize, String)>,
    tags: Vec<(usize, usize)>,
    headings: Vec<(usize, u8)>,
    blocks: Vec<(usize, usize)>,
}

fn found_by_styles(text: &str) -> Found {
    let starts = line_starts(text);
    let mut found = Found::default();
    for (line, style) in styled(text).iter().enumerate() {
        let start = starts[line];
        for link in &style.links {
            found.links.push((
                start + link.range.start,
                start + link.range.end,
                link.target.clone(),
            ));
        }
        if let LineKind::Heading(level) = style.kind {
            found.headings.push((line, level));
        }
        if style.kind == LineKind::Frontmatter {
            continue;
        }
        for span in &style.spans {
            let range = (start + span.range.start, start + span.range.end);
            match span.style.role {
                Role::Tag => found.tags.push(range),
                Role::BlockId => found.blocks.push(range),
                _ => {}
            }
        }
    }
    found
}

fn found_by_scan(text: &str) -> Found {
    let scan = scan(text);
    let starts = line_starts(text);
    let line_of = |at: usize| starts.partition_point(|&start| start <= at) - 1;
    Found {
        links: scan
            .links
            .iter()
            .map(|link| {
                let target = link.target_text(text).into_owned();
                (link.span.start, link.span.end, target)
            })
            .collect(),
        tags: scan
            .tags
            .iter()
            .map(|tag| (tag.span.start, tag.span.end))
            .collect(),
        headings: scan
            .headings
            .iter()
            .map(|heading| (line_of(heading.span.start), heading.level))
            .collect(),
        blocks: scan
            .blocks
            .iter()
            .map(|block| (block.span.start, block.span.end))
            .collect(),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2_000))]

    /// El editor y el índice ven los mismos links, tags, headings y bloques.
    #[test]
    fn styles_agree_with_the_scan(text in note()) {
        prop_assert_eq!(found_by_styles(&text), found_by_scan(&text));
    }

    /// Editar y pedir de a poco da lo mismo que calcular todo de cero.
    #[test]
    fn incremental_equals_from_scratch(
        text in note(),
        edits in prop::collection::vec((any::<prop::sample::Index>(), 0..3usize, note(), any::<prop::sample::Index>()), 1..6),
    ) {
        let mut text = text;
        let mut styles = Styles::new(TextLines::new(&text).line_count());
        styles.ensure(usize::MAX, &TextLines::new(&text));
        for (at, removed, inserted, asked) in edits {
            let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
            let first = at.index(lines.len());
            let removed = removed.min(lines.len() - first);
            let new: Vec<String> = inserted.split('\n').map(str::to_owned).collect();
            let inserted = new.len();
            lines.splice(first..first + removed, new);
            text = lines.join("\n");
            styles.edit(first, removed, inserted);
            let source = TextLines::new(&text);
            styles.ensure(asked.index(source.line_count()), &source);
        }
        let source = TextLines::new(&text);
        styles.ensure(usize::MAX, &source);
        let incremental: Vec<_> = (0..source.line_count())
            .map(|line| styles.line(line).unwrap().clone())
            .collect();
        prop_assert_eq!(incremental, styled(&text));
    }
}

/// El tag que se está escribiendo, como lo ve el estilo: con el cursor del
/// `#` al byte después del nombre, y nunca en código ni en el frontmatter.
#[rstest::rstest]
#[case::al_final("hay #rust", 9, Some(5..9))]
#[case::sobre_el_numeral("hay #rust", 4, Some(5..9))]
#[case::en_medio("hay #rust y más", 6, Some(5..9))]
#[case::fuera("hay #rust y más", 10, None)]
#[case::en_cursiva("*a #rust* y", 8, Some(4..8))]
#[case::pegado_a_otro_estilo("**#rust** y", 7, None)]
#[case::hasta_donde_corta_otro_estilo("#ru**st** y", 3, Some(1..3))]
#[case::en_codigo_inline("`#rust`", 5, None)]
#[case::heading("# rust", 6, None)]
#[case::solo_el_numeral("#", 1, None)]
fn tag_at_line(
    #[case] line: &str,
    #[case] at: usize,
    #[case] name: Option<std::ops::Range<usize>>,
) {
    let style = &styled(line)[0];
    assert_eq!(style.tag_at(at).map(|spot| spot.name), name);
}

#[test]
fn tag_at_no_mira_el_frontmatter_ni_el_codigo() {
    let text = "---\ntags: [rust]\n---\n```\n#rust\n```\n";
    let styles = styled(text);
    assert_eq!(styles[1].tag_at(8), None);
    assert_eq!(styles[4].tag_at(5), None);
}

/// El wikilink que se está escribiendo: de su `[[` al cursor, cerrado o no,
/// y nunca en código, fórmulas, comentarios ni el frontmatter.
#[rstest::rstest]
#[case::recien_abierto("ver [[", 6, Some((6..6, false)))]
#[case::a_medio_escribir("ver [[pla", 9, Some((6..9, false)))]
#[case::con_texto_despues("ver [[pla y sigue", 9, Some((6..9, false)))]
#[case::un_embed("![[foto", 7, Some((3..7, false)))]
#[case::cerrado_con_el_cursor_al_final("[[plan]] y", 6, Some((2..6, true)))]
#[case::cerrado_antes_de_la_subruta("[[plan#Meta]]", 6, Some((2..6, true)))]
#[case::cerrado_antes_del_alias("[[plan|el plan]]", 6, Some((2..6, true)))]
#[case::en_medio_del_destino("[[plan]]", 4, None)]
#[case::en_la_subruta("[[plan#Me", 9, None)]
#[case::en_el_alias("[[plan|el", 9, None)]
#[case::tras_cerrarlo("[[plan]] y", 10, None)]
#[case::el_ultimo_de_la_linea("[[a]] y [[b", 11, Some((10..11, false)))]
#[case::sin_corchetes("plan", 4, None)]
#[case::un_solo_corchete("[pla", 4, None)]
#[case::en_codigo_inline("`[[pla` y", 6, None)]
#[case::en_una_formula("$[[x$ y", 4, None)]
#[case::en_un_comentario("%%[[pla%% y", 7, None)]
#[case::con_acentos("[[canción", 10, Some((2..10, false)))]
#[case::fuera_de_un_caracter("[[ñ", 3, None)]
#[case::escapado("\\[[pla", 6, None)]
#[case::la_barra_escapada("\\\\[[pla", 7, Some((4..7, false)))]
#[case::en_codigo_tras_el_corchete("[[pla `co` y", 8, None)]
#[case::en_un_comentario_tras_el_corchete("[[pla %%co%% y", 9, None)]
#[case::en_el_destino_de_un_link_markdown("[x]([[a]]) y", 7, None)]
#[case::en_el_texto_de_un_link_markdown("[ver [[a]]](b.md) y", 8, None)]
fn typed_wikilink_line(
    #[case] line: &str,
    #[case] at: usize,
    #[case] typed: Option<(std::ops::Range<usize>, bool)>,
) {
    let style = &styled(line)[0];
    let found = style
        .typed_wikilink(line, at)
        .map(|typed| (typed.target, typed.closed));
    assert_eq!(found, typed);
}

#[test]
fn typed_wikilink_no_mira_el_frontmatter_ni_un_bloque_de_codigo() {
    let text = "---\nup: [[pla\n---\n```\n[[pla\n```\n";
    let styles = styled(text);
    let lines: Vec<&str> = text.split('\n').collect();
    assert_eq!(styles[1].typed_wikilink(lines[1], 9), None);
    assert_eq!(styles[4].typed_wikilink(lines[4], 5), None);
}
