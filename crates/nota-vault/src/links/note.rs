//! De una nota escaneada a sus links indexados, sin resolver todavía.

use std::ops::Range;

use nota_syntax::{Link, LinkSyntax, Subpath};

use super::{IndexedLink, LinkContext, LinkSubpath, LinkTarget};

/// Cuánto texto de la línea se guarda a cada lado del link, en bytes.
const CONTEXT_BYTES: usize = 80;

/// Los links de `text`, en orden de aparición y con `LinkTarget::Unresolved`.
pub(super) fn scan_links(text: &str) -> Vec<IndexedLink> {
    let mut line = 0;
    let mut counted = 0;
    nota_syntax::scan(text)
        .links
        .into_iter()
        .map(|link| {
            line += text[counted..link.span.start].matches('\n').count();
            counted = link.span.start;
            indexed(text, &link, line)
        })
        .collect()
}

fn indexed(text: &str, link: &Link, line_number: usize) -> IndexedLink {
    IndexedLink {
        syntax: link.syntax,
        embed: link.embed,
        span: link.span.clone(),
        target_span: link.target.clone(),
        target_text: link.target_text(text).into_owned(),
        subpath: subpath(text, link),
        line: line_number,
        context: context(text, &link.span),
        bracketed: link.syntax == LinkSyntax::Markdown
            && text[link.span.start..link.target.start]
                .trim_end()
                .ends_with('<'),
        target: LinkTarget::Unresolved,
    }
}

pub(super) fn subpath(text: &str, link: &Link) -> Option<LinkSubpath> {
    let name = link.subpath_text(text)?.into_owned();
    link.subpath.as_ref().map(|subpath| match subpath {
        Subpath::Heading(_) => LinkSubpath::Heading(name),
        Subpath::Block(_) => LinkSubpath::Block(name),
    })
}

/// La línea del link, recortada a [`CONTEXT_BYTES`] a cada lado. Un link
/// nunca cruza un salto de línea, así que siempre entra entero.
fn context(text: &str, span: &Range<usize>) -> LinkContext {
    let line_start = text[..span.start].rfind('\n').map_or(0, |i| i + 1);
    let line_end = text[span.end..]
        .find('\n')
        .map_or(text.len(), |i| span.end + i);
    let line_end = if text[..line_end].ends_with('\r') {
        line_end - 1
    } else {
        line_end
    }
    .max(span.end);
    let start = floor_boundary(
        text,
        span.start.saturating_sub(CONTEXT_BYTES).max(line_start),
    );
    let end = ceil_boundary(text, (span.end + CONTEXT_BYTES).min(line_end));
    LinkContext {
        text: text[start..end].to_owned(),
        link: span.start - start..span.end - start,
    }
}

fn floor_boundary(text: &str, mut at: usize) -> usize {
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

fn ceil_boundary(text: &str, mut at: usize) -> usize {
    while !text.is_char_boundary(at) {
        at += 1;
    }
    at
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn line_and_context_of_each_link() {
        let text = "uno\r\ndos [[a]] tres\n\n[x](b.md) y `[[c]]`";
        let links = scan_links(text);
        let shown: Vec<(usize, &str, &str)> = links
            .iter()
            .map(|link| {
                let context = &link.context;
                (
                    link.line,
                    context.text.as_str(),
                    &context.text[context.link.clone()],
                )
            })
            .collect();
        assert_eq!(
            shown,
            [
                (1, "dos [[a]] tres", "[[a]]"),
                (3, "[x](b.md) y `[[c]]`", "[x](b.md)")
            ]
        );
    }

    #[test]
    fn a_long_line_is_cut_on_char_boundaries() {
        let text = format!("{}[[a]]{}", "é".repeat(100), "ñ".repeat(100));
        let link = &scan_links(&text)[0];
        assert!(link.context.text.len() <= 2 * CONTEXT_BYTES + "[[a]]".len() + 2);
        assert_eq!(&link.context.text[link.context.link.clone()], "[[a]]");
    }

    #[test]
    fn markdown_targets_are_decoded_and_brackets_remembered() {
        let links = scan_links("[a](<Mi Nota.md>) [b](Mi%20Nota.md#Sec%201) [[c#^id]]");
        assert_eq!(links[0].target_text, "Mi Nota.md");
        assert!(links[0].bracketed);
        assert_eq!(links[1].target_text, "Mi Nota.md");
        assert!(!links[1].bracketed);
        assert_eq!(
            links[1].subpath,
            Some(LinkSubpath::Heading("Sec 1".to_owned()))
        );
        assert_eq!(links[2].subpath, Some(LinkSubpath::Block("id".to_owned())));
    }
}
