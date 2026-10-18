//! Cómo se ven los resultados del scan en los snapshots y en las tablas.

use std::fmt::Write as _;
use std::ops::Range;

use nota_syntax::{Link, LinkSyntax, Problem, Scan, Subpath, Value};

/// El texto de cada link y tag encontrado, en orden de aparición.
pub fn found(text: &str, scan: &Scan) -> Vec<String> {
    let links = scan.links.iter().map(|link| &link.span);
    let tags = scan.tags.iter().map(|tag| &tag.span);
    let mut spans: Vec<&Range<usize>> = links.chain(tags).collect();
    spans.sort_by_key(|span| span.start);
    spans
        .into_iter()
        .map(|span| text[span.clone()].to_owned())
        .collect()
}

/// El texto de cada valor del frontmatter.
pub fn texts(values: &[Value]) -> Vec<&str> {
    values.iter().map(|value| value.text.as_str()).collect()
}

/// Un valor con lo que ocupa en la nota, lo que reescribe y lo que se borra
/// para sacarlo de su lista.
fn value_line(text: &str, value: &Value) -> String {
    let literal = value.literal.as_ref().map_or_else(
        || "no literal".to_owned(),
        |range| format!("literal {range:?}"),
    );
    format!(
        "{:?} en {} {literal} quita {:?}",
        value.text,
        part(text, &value.span),
        &text[value.removal.clone()]
    )
}

fn part(text: &str, range: &Range<usize>) -> String {
    format!("{range:?} {:?}", &text[range.clone()])
}

fn link_line(text: &str, link: &Link) -> String {
    let kind = match (link.syntax, link.embed) {
        (LinkSyntax::Wiki, false) => "wikilink",
        (LinkSyntax::Wiki, true) => "embed",
        (LinkSyntax::Markdown, false) => "markdown",
        (LinkSyntax::Markdown, true) => "markdown embed",
    };
    let mut out = format!(
        "{kind} {}\n    target {}",
        part(text, &link.span),
        part(text, &link.target)
    );
    match &link.subpath {
        Some(Subpath::Heading(range)) => write!(out, "\n    heading {}", part(text, range)),
        Some(Subpath::Block(range)) => write!(out, "\n    block {}", part(text, range)),
        None => Ok(()),
    }
    .unwrap();
    if let Some(alias) = &link.alias {
        write!(out, "\n    alias {}", part(text, alias)).unwrap();
    }
    let decoded = link.target_text(text);
    if decoded != text[link.target.clone()] {
        write!(out, "\n    decoded {decoded:?}").unwrap();
    }
    out
}

/// Todo lo encontrado, con sus rangos y el texto de cada uno.
pub fn render(text: &str, scan: &Scan) -> String {
    let mut out = String::new();
    if let Some(frontmatter) = &scan.frontmatter {
        writeln!(
            out,
            "frontmatter {:?} yaml {:?}",
            frontmatter.span, frontmatter.yaml
        )
        .unwrap();
        let lists = [
            ("tags", &frontmatter.tags),
            ("aliases", &frontmatter.aliases),
            ("parents", &frontmatter.parents),
            ("cssclasses", &frontmatter.cssclasses),
        ];
        let singles = [("color", &frontmatter.color), ("icon", &frontmatter.icon)];
        let fields = lists
            .into_iter()
            .flat_map(|(field, values)| values.iter().map(move |value| (field, value)))
            .chain(
                singles
                    .into_iter()
                    .filter_map(|(field, value)| Some((field, value.as_ref()?))),
            );
        for (field, value) in fields {
            writeln!(out, "    {field} {}", value_line(text, value)).unwrap();
        }
        match &frontmatter.entries {
            Some(entries) => {
                for entry in entries {
                    writeln!(out, "    entry {} {}", entry.key, part(text, &entry.span)).unwrap();
                }
            }
            None => writeln!(out, "    entries: no se puede editar").unwrap(),
        }
    }
    for problem in &scan.problems {
        writeln!(out, "problem {problem}").unwrap();
        if let Problem::InvalidYaml { at, .. } = problem {
            let line = text[..*at].matches('\n').count() + 1;
            writeln!(out, "    at {at} (línea {line})").unwrap();
        }
    }
    for link in &scan.links {
        writeln!(out, "{}", link_line(text, link)).unwrap();
    }
    for tag in &scan.tags {
        writeln!(
            out,
            "tag {} name {:?}",
            part(text, &tag.span),
            tag.name(text)
        )
        .unwrap();
    }
    for heading in &scan.headings {
        writeln!(
            out,
            "heading {} {}\n    text {}",
            heading.level,
            part(text, &heading.span),
            part(text, &heading.text)
        )
        .unwrap();
    }
    for block in &scan.blocks {
        writeln!(out, "block {}", part(text, &block.span)).unwrap();
    }
    out
}
