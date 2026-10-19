use std::ops::Range;

use metalmind_syntax::{Link, Problem, Subpath, scan};
use proptest::prelude::*;

/// Trozos que arman sintaxis de Metalmind a medias: la idea es dar con los
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
        Just("$".to_owned()),
        Just("$$".to_owned()),
        Just("\\$".to_owned()),
        Just("5".to_owned()),
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

/// Un valor de `tags:` escrito de alguna de las formas que permite YAML.
fn written_tag() -> impl Strategy<Value = (String, String)> {
    let name = prop::sample::select(vec!["rust", "Ñandú", "2024", "a-b", "🦀"]);
    (name, 0..4_usize).prop_map(|(name, style)| {
        let written = match style {
            0 => name.to_owned(),
            1 => format!("\"{name}\""),
            2 => format!("'#{name}'"),
            _ => format!("\" {name} \""),
        };
        (name.to_owned(), written)
    })
}

/// Un frontmatter con `tags:` en una lista entre corchetes (en una línea o
/// en varias) o en bloque.
fn tags_frontmatter() -> impl Strategy<Value = (Vec<String>, String)> {
    (
        prop::collection::vec(written_tag(), 1..6),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(|(tags, block, crlf, multiline)| {
            let eol = if crlf { "\r\n" } else { "\n" };
            let written: Vec<&str> = tags.iter().map(|(_, written)| written.as_str()).collect();
            let list = if block {
                written.iter().fold(String::new(), |mut list, tag| {
                    list.push_str(eol);
                    list.push_str("  - ");
                    list.push_str(tag);
                    list
                })
            } else if multiline {
                format!(" [{}]", written.join(&format!(",{eol}  ")))
            } else {
                format!(" [{}]", written.join(", "))
            };
            let text = format!("---{eol}title: x{eol}tags:{list}{eol}otro: 1{eol}---{eol}#cuerpo");
            (tags.into_iter().map(|(name, _)| name).collect(), text)
        })
}

fn tag_names(text: &str) -> Vec<String> {
    scan(text)
        .frontmatter
        .map(|frontmatter| frontmatter.tags.into_iter().map(|tag| tag.text).collect())
        .unwrap_or_default()
}

proptest! {
    /// Lo que dice `literal` es el nombre, y quitar un valor por su
    /// `removal` deja el YAML válido con los demás, en orden.
    #[test]
    fn frontmatter_values_can_be_rewritten_and_removed((names, text) in tags_frontmatter()) {
        let scan = scan(&text);
        prop_assert_eq!(&scan.problems, &[]);
        let tags = scan.frontmatter.map(|frontmatter| frontmatter.tags).unwrap_or_default();
        prop_assert_eq!(tag_names(&text), names.clone());
        for (which, tag) in tags.iter().enumerate() {
            if let Some(literal) = &tag.literal {
                prop_assert_eq!(&text[literal.clone()], tag.text.as_str());
            }
            let rest = format!("{}{}", &text[..tag.removal.start], &text[tag.removal.end..]);
            let mut expected = names.clone();
            expected.remove(which);
            prop_assert_eq!(tag_names(&rest), expected, "{:?}", rest);
        }
    }

    /// Quitar varios valores a la vez: los que tienen borrados todos los de
    /// antes van por `removal_first` y los demás por `removal`. Los rangos
    /// nunca se pisan y quedan los demás, en orden.
    #[test]
    fn several_values_can_be_removed_at_once(
        (names, text) in tags_frontmatter(),
        removed in prop::collection::vec(any::<bool>(), 6),
    ) {
        let tags = scan(&text).frontmatter.map(|frontmatter| frontmatter.tags).unwrap_or_default();
        let mut prefix = true;
        let mut ranges = Vec::new();
        let mut expected = Vec::new();
        for ((tag, name), &remove) in tags.iter().zip(&names).zip(&removed) {
            if remove {
                ranges.push(if prefix { tag.removal_first.clone() } else { tag.removal.clone() });
            } else {
                prefix = false;
                expected.push(name.clone());
            }
        }
        let mut rest = String::new();
        let mut at = 0;
        for range in &ranges {
            prop_assert!(range.start >= at, "{:?} se pisa en {:?}", ranges, text);
            rest.push_str(&text[at..range.start]);
            at = range.end;
        }
        rest.push_str(&text[at..]);
        prop_assert_eq!(scan(&rest).problems, vec![], "{:?}", rest);
        prop_assert_eq!(tag_names(&rest), expected, "{:?}", rest);
    }

    /// Quitar el campo `tags` entero por su `Entry` deja los demás campos
    /// byte a byte, y el frontmatter sigue siendo editable.
    #[test]
    fn a_whole_field_can_be_removed((_names, text) in tags_frontmatter()) {
        let frontmatter = scan(&text).frontmatter.unwrap();
        let keys: Vec<&str> = frontmatter.entries.iter().flatten().map(|entry| entry.key.as_str()).collect();
        prop_assert_eq!(keys, ["title", "tags", "otro"]);
        let entry = frontmatter.entry("tags").unwrap();
        let rest = format!("{}{}", &text[..entry.span.start], &text[entry.span.end..]);
        let left = scan(&rest).frontmatter.unwrap();
        let keys: Vec<&str> = left.entries.iter().flatten().map(|entry| entry.key.as_str()).collect();
        prop_assert_eq!(keys, ["title", "otro"]);
        prop_assert!(left.tags.is_empty());
        prop_assert!(rest.contains("title: x") && rest.contains("otro: 1"));
    }

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
