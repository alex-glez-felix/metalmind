//! Agregar tags al `tags:` del frontmatter de una nota, tocando solo ese
//! campo: al crearla en focus de tags (05 §3) o al etiquetarla desde la
//! lista de notas.

use std::ops::Range;

use metalmind_syntax::{Frontmatter, Problem, scan};
use metalmind_vault::tags::{TagKey, tag_name, yaml_scalar};

use field::{Field, tags_field};

mod field;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TagsNotAdded {
    #[error("el frontmatter de la nota tiene errores")]
    InvalidFrontmatter,
    #[error("`{0}` no es un tag válido")]
    InvalidTag(String),
    #[error("el campo `tags` de la nota no está escrito en bloque, línea por línea")]
    UnsupportedLayout,
}

/// `text` con `tags` en su frontmatter, después de los que ya tenga. Sin
/// frontmatter, se le agrega uno. Una lista se alarga en su estilo, de flujo
/// o en bloque, sin tocar sus comentarios. Si algo falla, `text` no se toca.
///
/// # Errors
/// Si un tag no es válido, el frontmatter tiene errores o su `tags` no se
/// puede alargar sin reescribirlo.
pub fn with_tags(text: &str, tags: &[String]) -> Result<String, TagsNotAdded> {
    let wanted = wanted(tags)?;
    if wanted.is_empty() {
        return Ok(text.to_owned());
    }
    let scanned = scan(text);
    let edited = match scanned.frontmatter {
        None => with_new_frontmatter(text, &wanted),
        Some(frontmatter) => {
            if !is_clean(&scanned.problems) {
                return Err(TagsNotAdded::InvalidFrontmatter);
            }
            let present: Vec<TagKey> = frontmatter
                .tags
                .iter()
                .map(|tag| TagKey::new(&tag.text))
                .collect();
            let missing: Vec<&str> = wanted
                .iter()
                .copied()
                .filter(|tag| !present.contains(&TagKey::new(tag)))
                .collect();
            if missing.is_empty() {
                return Ok(text.to_owned());
            }
            with_field_extended(text, &frontmatter, &missing)
        }
    };
    if has_all_tags(&edited, &wanted) {
        Ok(edited)
    } else {
        Err(TagsNotAdded::UnsupportedLayout)
    }
}

/// Los nombres sin `#` y sin repetir, o el primero que no es un tag.
fn wanted(tags: &[String]) -> Result<Vec<&str>, TagsNotAdded> {
    let mut wanted: Vec<&str> = Vec::new();
    for tag in tags {
        let name = tag_name(tag).ok_or_else(|| TagsNotAdded::InvalidTag(tag.clone()))?;
        if !wanted
            .iter()
            .any(|seen| TagKey::new(seen) == TagKey::new(name))
        {
            wanted.push(name);
        }
    }
    Ok(wanted)
}

/// Un `template:` que no es texto no impide agregar tags.
fn is_clean(problems: &[Problem]) -> bool {
    problems
        .iter()
        .all(|problem| matches!(problem, Problem::NotText { .. }))
}

fn with_new_frontmatter(text: &str, tags: &[&str]) -> String {
    let bom = if text.starts_with('\u{FEFF}') {
        '\u{FEFF}'.len_utf8()
    } else {
        0
    };
    let (bom, body) = text.split_at(bom);
    format!("{bom}---\ntags: {}\n---\n{body}", flow_list(tags))
}

fn with_field_extended(text: &str, frontmatter: &Frontmatter, missing: &[&str]) -> String {
    let newline = if text[frontmatter.span.clone()].contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let base = frontmatter.yaml.start;
    let yaml = &text[frontmatter.yaml.clone()];
    let Some(field) = tags_field(yaml) else {
        let line = format!("tags: {}{newline}", flow_list(missing));
        return splice(text, base..base, &line);
    };
    let value = field.value.clone();
    let edit = match yaml[value.clone()].trim() {
        "" if !field.continues => Some(Edit::Replace(value, format!(" {}", flow_list(missing)))),
        "" => block_items(yaml, &field, missing, newline),
        inline if inline.starts_with('[') && inline.ends_with(']') => {
            flow_items(yaml, value, missing)
        }
        _ => None,
    }
    .unwrap_or_else(|| {
        let all: Vec<&str> = frontmatter
            .tags
            .iter()
            .map(|tag| tag.text.as_str())
            .chain(missing.iter().copied())
            .collect();
        let comment = &yaml[field.comment.clone()];
        let line = format!("tags: {}{comment}{newline}", flow_list(&all));
        Edit::Replace(field.whole.clone(), line)
    });
    let (range, with) = match edit {
        Edit::Replace(range, with) => (range, with),
        Edit::Insert(at, with) => (at..at, with),
    };
    splice(text, offset(&range, base), &with)
}

enum Edit {
    Replace(Range<usize>, String),
    Insert(usize, String),
}

/// `- tag` después del último elemento, con su sangría.
fn block_items(yaml: &str, field: &Field, missing: &[&str], newline: &str) -> Option<Edit> {
    let last = field.items.last()?;
    let line = &yaml[last.clone()];
    let indent = line.len() - line.trim_start_matches([' ', '\t']).len();
    let after_dash = &line[indent + 1..];
    let gap = after_dash.len() - after_dash.trim_start_matches([' ', '\t']).len();
    let prefix = format!(
        "{}- {}",
        &line[..indent],
        &after_dash[..gap.saturating_sub(1)]
    );
    let mut items = String::new();
    if !yaml[..field.whole.end].ends_with('\n') {
        items.push_str(newline);
    }
    items.extend(
        missing
            .iter()
            .map(|tag| format!("{prefix}{}{newline}", yaml_scalar(tag))),
    );
    Some(Edit::Insert(field.whole.end, items))
}

/// `, tag` tras el último elemento de una lista de flujo de una sola línea.
fn flow_items(yaml: &str, value: Range<usize>, missing: &[&str]) -> Option<Edit> {
    let close = value.start + yaml[value.clone()].rfind(']')?;
    let inner = &yaml[value.start..close];
    let open = value.start + inner.find('[')?;
    let items = &yaml[open + 1..close];
    let joined = missing
        .iter()
        .map(|tag| yaml_scalar(tag))
        .collect::<Vec<_>>();
    if items.trim().is_empty() {
        return Some(Edit::Replace(open + 1..close, joined.join(", ")));
    }
    // Antes de una coma final, que YAML permite: `[a,]` queda `[a, b,]`.
    let last = items.trim_end();
    let end = open + 1 + last.strip_suffix(',').unwrap_or(last).trim_end().len();
    Some(Edit::Insert(end, format!(", {}", joined.join(", "))))
}

fn flow_list(tags: &[&str]) -> String {
    let tags: Vec<String> = tags.iter().map(|tag| yaml_scalar(tag)).collect();
    format!("[{}]", tags.join(", "))
}

fn splice(text: &str, range: Range<usize>, with: &str) -> String {
    let mut edited = String::with_capacity(text.len() + with.len());
    edited.push_str(&text[..range.start]);
    edited.push_str(with);
    edited.push_str(&text[range.end..]);
    edited
}

fn offset(range: &Range<usize>, by: usize) -> Range<usize> {
    range.start + by..range.end + by
}

fn has_all_tags(text: &str, tags: &[&str]) -> bool {
    let scanned = scan(text);
    is_clean(&scanned.problems)
        && scanned.frontmatter.is_some_and(|frontmatter| {
            tags.iter().all(|tag| {
                frontmatter
                    .tags
                    .iter()
                    .any(|found| TagKey::new(&found.text) == TagKey::new(tag))
            })
        })
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

    fn added(text: &str, tags: &[&str]) -> Result<String, TagsNotAdded> {
        let tags: Vec<String> = tags.iter().map(|&tag| tag.to_owned()).collect();
        with_tags(text, &tags)
    }

    #[rstest]
    #[case::no_tags("# Nota\n", &[], "# Nota\n")]
    #[case::no_frontmatter("# Nota\n", &["a", "b"], "---\ntags: [a, b]\n---\n# Nota\n")]
    #[case::empty_note("", &["a"], "---\ntags: [a]\n---\n")]
    #[case::bom("\u{FEFF}# N", &["a"], "\u{FEFF}---\ntags: [a]\n---\n# N")]
    #[case::hash_and_duplicates("", &["#a", "A", "a"], "---\ntags: [a]\n---\n")]
    #[case::same_tag_by_folding("", &["straße", "STRASSE"], "---\ntags: [straße]\n---\n")]
    #[case::without_tags_key(
        "---\ncreado: hoy # comentario\n---\n",
        &["a"],
        "---\ntags: [a]\ncreado: hoy # comentario\n---\n"
    )]
    #[case::already_there("---\ntags: [A]\n---\n", &["a"], "---\ntags: [A]\n---\n")]
    #[case::flow_list(
        "---\ntipo: x\ntags: [reunion]\notro: y\n---\n",
        &["trabajo"],
        "---\ntipo: x\ntags: [reunion, trabajo]\notro: y\n---\n"
    )]
    #[case::flow_list_keeps_its_spacing_and_comment(
        "---\ntags: [ reunion ]  # de la plantilla\n---\n",
        &["trabajo", "b"],
        "---\ntags: [ reunion, trabajo, b ]  # de la plantilla\n---\n"
    )]
    #[case::flow_list_with_trailing_comma(
        "---\ntags: [a ,  ]\n---\n",
        &["b"],
        "---\ntags: [a, b ,  ]\n---\n"
    )]
    #[case::empty_flow_list("---\ntags: [] # vacía\n---\n", &["a"], "---\ntags: [a] # vacía\n---\n")]
    #[case::block_list(
        "---\ntags:\n  - reunion # la primera\n  - '#semanal'\n# fin de tags\notro: y\n---\n",
        &["trabajo"],
        "---\ntags:\n  - reunion # la primera\n  - '#semanal'\n  - trabajo\n# fin de tags\notro: y\n---\n"
    )]
    #[case::block_list_at_column_zero(
        "---\ntags:\n- reunion\n---\n",
        &["trabajo"],
        "---\ntags:\n- reunion\n- trabajo\n---\n"
    )]
    #[case::block_list_wide_dash(
        "---\ntags:\n    -   reunion\n---\n",
        &["trabajo"],
        "---\ntags:\n    -   reunion\n    -   trabajo\n---\n"
    )]
    #[case::text_value(
        "---\ntags: reunion # sola\n---\n",
        &["x"],
        "---\ntags: [reunion, x] # sola\n---\n"
    )]
    #[case::empty_value(
        "---\ntags: # ninguno\notro: 1\n---\n",
        &["x"],
        "---\ntags: [x] # ninguno\notro: 1\n---\n"
    )]
    #[case::multiline_flow_list(
        "---\ntags: [\n  a,\n  b\n  ]\n---\n",
        &["c"],
        "---\ntags: [a, b, c]\n---\n"
    )]
    #[case::unclosed_fence("---\ntags: [a]", &["b"], "---\ntags: [b]\n---\n---\ntags: [a]")]
    #[case::quoted_key("---\n\"tags\": [a]\n---\n", &["b"], "---\n\"tags\": [a, b]\n---\n")]
    #[case::crlf(
        "---\r\ntags:\r\n  - a\r\notro: 1\r\n---\r\n",
        &["b"],
        "---\r\ntags:\r\n  - a\r\n  - b\r\notro: 1\r\n---\r\n"
    )]
    #[case::unicode("---\ntags: [ñandú]\n---\n", &["über"], "---\ntags: [ñandú, über]\n---\n")]
    #[case::nested_key_is_not_tags(
        "---\nmeta:\n  tags: [x]\n---\n",
        &["a"],
        "---\ntags: [a]\nmeta:\n  tags: [x]\n---\n"
    )]
    #[case::null_and_numbers_are_quoted(
        "",
        &["null", "007", "1e3", "-0", "2024-10"],
        "---\ntags: [\"null\", \"007\", \"1e3\", \"-0\", \"2024-10\"]\n---\n"
    )]
    #[case::quoted_in_a_block_list(
        "---\ntags:\n  - a\n---\n",
        &["NULL"],
        "---\ntags:\n  - a\n  - \"NULL\"\n---\n"
    )]
    #[case::quoted_text_and_comment(
        "---\ntags: \"a\" # b\n---\n",
        &["c"],
        "---\ntags: [a, c] # b\n---\n"
    )]
    #[case::text_over_two_lines(
        "---\ntags:\n  a\notro: 1\n---\n",
        &["c"],
        "---\ntags: [a, c]\notro: 1\n---\n"
    )]
    fn adds_tags(#[case] text: &str, #[case] tags: &[&str], #[case] expected: &str) {
        assert_eq!(added(text, tags).as_deref(), Ok(expected));
    }

    #[rstest]
    #[case::broken_yaml("---\ntags: [a\n---\n", &["b"], TagsNotAdded::InvalidFrontmatter)]
    #[case::invalid_tag_in_template("---\ntags: [a b, c]\n---\n", &["x"], TagsNotAdded::InvalidFrontmatter)]
    #[case::invalid_tag("", &["con espacio"], TagsNotAdded::InvalidTag("con espacio".to_owned()))]
    #[case::flow_mapping("---\n{tags: [a]}\n---\n", &["b"], TagsNotAdded::UnsupportedLayout)]
    fn leaves_the_text_alone(
        #[case] text: &str,
        #[case] tags: &[&str],
        #[case] error: TagsNotAdded,
    ) {
        assert_eq!(added(text, tags), Err(error));
    }
}
