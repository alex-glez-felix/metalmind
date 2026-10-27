//! De una nota escaneada a sus links indexados, sin resolver todavía.

use std::ops::Range;

use metalmind_syntax::{Link, LinkSyntax, Subpath};

use super::{IndexedLink, LinkContext, LinkSubpath, LinkTarget};

/// Cuánto texto de la línea se guarda a cada lado del link, en bytes.
const CONTEXT_BYTES: usize = 80;

/// Los links de `text`, en orden de aparición y con `LinkTarget::Unresolved`.
pub(super) fn scan_links(text: &str) -> Vec<IndexedLink> {
    let mut lines = Lines::new(text);
    metalmind_syntax::scan(text)
        .links
        .into_iter()
        .map(|link| {
            let (number, line) = lines.at(link.span.start);
            indexed(text, &link, number, &line)
        })
        .collect()
}

/// Las líneas de un texto, recorridas hacia adelante: cada línea se busca
/// una vez aunque tenga muchos links.
struct Lines<'t> {
    text: &'t str,
    number: usize,
    start: usize,
    end: Option<usize>,
    counted: usize,
}

impl<'t> Lines<'t> {
    fn new(text: &'t str) -> Self {
        Self {
            text,
            number: 0,
            start: 0,
            end: None,
            counted: 0,
        }
    }

    /// El número, desde cero, y el rango de la línea de `offset`, que no
    /// puede ser menor que el de la llamada anterior.
    fn at(&mut self, offset: usize) -> (usize, Range<usize>) {
        let skipped = &self.text[self.counted..offset];
        if let Some(last) = skipped.rfind('\n') {
            self.number += skipped.matches('\n').count();
            self.start = self.counted + last + 1;
            self.end = None;
        }
        self.counted = offset;
        let text = self.text;
        let end = *self
            .end
            .get_or_insert_with(|| text[offset..].find('\n').map_or(text.len(), |i| offset + i));
        (self.number, self.start..end)
    }
}

fn indexed(text: &str, link: &Link, number: usize, bounds: &Range<usize>) -> IndexedLink {
    IndexedLink {
        syntax: link.syntax,
        embed: link.embed,
        span: link.span.clone(),
        target_span: link.target.clone(),
        target_text: link.target_text(text).into_owned(),
        subpath: subpath(text, link),
        line: number,
        column: link.span.start - bounds.start,
        context: context(text, &link.span, bounds),
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
fn context(text: &str, span: &Range<usize>, line: &Range<usize>) -> LinkContext {
    let line_start = line.start;
    let line_end = line.end;
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
    fn line_column_and_context_of_each_link() {
        let text = "uno\r\ndos [[a]] tres\n\n[x](b.md) y `[[c]]`";
        let links = scan_links(text);
        let shown: Vec<(usize, usize, &str, &str)> = links
            .iter()
            .map(|link| {
                let context = &link.context;
                (
                    link.line,
                    link.column,
                    context.text.as_str(),
                    &context.text[context.link.clone()],
                )
            })
            .collect();
        assert_eq!(
            shown,
            [
                (1, 4, "dos [[a]] tres", "[[a]]"),
                (3, 0, "[x](b.md) y `[[c]]`", "[x](b.md)")
            ]
        );
    }

    #[test]
    fn a_long_line_is_cut_on_char_boundaries() {
        let text = format!("{}[[a]]{}", "é".repeat(100), "ñ".repeat(100));
        let link = &scan_links(&text)[0];
        assert_eq!(link.column, 200, "en bytes");
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
