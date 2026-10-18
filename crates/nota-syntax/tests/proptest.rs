use std::ops::Range;

use nota_syntax::{Link, Problem, Subpath, scan};
use proptest::prelude::*;

/// Trozos que arman sintaxis de Obsidian a medias: la idea es dar con los
/// bordes (cierres que faltan, saltos de línea en medio, multibyte).
fn piece() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("[[".to_owned()),
        Just("]]".to_owned()),
        Just("![[".to_owned()),
        Just("[".to_owned()),
        Just("](".to_owned()),
        Just(")".to_owned()),
        Just("<".to_owned()),
        Just(">".to_owned()),
        Just("|".to_owned()),
        Just("\\|".to_owned()),
        Just("\\".to_owned()),
        Just("#".to_owned()),
        Just("#^".to_owned()),
        Just("`".to_owned()),
        Just("```".to_owned()),
        Just("~~~".to_owned()),
        Just("%%".to_owned()),
        Just("---".to_owned()),
        Just("\n".to_owned()),
        Just("\r\n".to_owned()),
        Just(" ".to_owned()),
        Just("\"".to_owned()),
        Just(":".to_owned()),
        Just("%2".to_owned()),
        Just("é".to_owned()),
        Just("🔥".to_owned()),
        Just("\u{200D}".to_owned()),
        Just("\u{FEFF}".to_owned()),
        Just("。".to_owned()),
        "[a-z0-9/.]{1,4}",
        any::<char>().prop_map(String::from),
    ]
}

fn note() -> impl Strategy<Value = String> {
    prop::collection::vec(piece(), 0..48).prop_map(|pieces| pieces.concat())
}

fn assert_range(
    text: &str,
    range: &Range<usize>,
    within: &Range<usize>,
) -> Result<(), TestCaseError> {
    prop_assert!(range.start <= range.end, "{range:?} al revés");
    prop_assert!(
        within.start <= range.start && range.end <= within.end,
        "{range:?} fuera de {within:?}"
    );
    prop_assert!(
        text.is_char_boundary(range.start) && text.is_char_boundary(range.end),
        "{range:?} parte un carácter"
    );
    Ok(())
}

fn assert_link(text: &str, link: &Link) -> Result<(), TestCaseError> {
    assert_range(text, &link.span, &(0..text.len()))?;
    assert_range(text, &link.target, &link.span)?;
    if let Some(Subpath::Heading(range) | Subpath::Block(range)) = &link.subpath {
        assert_range(text, range, &link.span)?;
    }
    if let Some(alias) = &link.alias {
        assert_range(text, alias, &link.span)?;
    }
    Ok(())
}

proptest! {
    #[test]
    fn spans_stay_on_char_boundaries(text in note()) {
        let scan = scan(&text);
        let whole = 0..text.len();
        if let Some(frontmatter) = &scan.frontmatter {
            assert_range(&text, &frontmatter.span, &whole)?;
            assert_range(&text, &frontmatter.yaml, &frontmatter.span)?;
        }
        for link in &scan.links {
            assert_link(&text, link)?;
        }
        for problem in &scan.problems {
            if let Problem::InvalidYaml { span, at, .. } = problem {
                assert_range(&text, &(*at..*at), span)?;
            }
        }
        for tag in &scan.tags {
            assert_range(&text, &tag.span, &whole)?;
            assert_range(&text, &tag.name, &tag.span)?;
        }
    }

    #[test]
    fn spans_are_ordered_and_disjoint(text in note()) {
        let scan = scan(&text);
        let mut spans: Vec<Range<usize>> = scan.links.iter().map(|l| l.span.clone())
            .chain(scan.tags.iter().map(|t| t.span.clone()))
            .collect();
        spans.sort_by_key(|span| span.start);
        for pair in spans.windows(2) {
            prop_assert!(pair[0].end <= pair[1].start, "{:?} se solapa con {:?}", pair[0], pair[1]);
        }
        if let (Some(frontmatter), Some(first)) = (&scan.frontmatter, spans.first()) {
            prop_assert!(frontmatter.span.end <= first.start);
        }
    }
}
