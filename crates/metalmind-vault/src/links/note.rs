//! De una nota escaneada a sus links indexados, sin resolver todavía.

use std::ops::Range;

use metalmind_syntax::{Link, LinkSyntax, Subpath};

use super::{IndexedLink, LinkContext, LinkSubpath, LinkTarget, Written};

/// Cuánto texto de la línea se guarda a cada lado del link, en bytes.
const CONTEXT_BYTES: usize = 80;

/// Los links de `text`, en orden de aparición y con `LinkTarget::Unresolved`.
pub(crate) fn scan_links(text: &str) -> Vec<IndexedLink> {
    links_of(text, &metalmind_syntax::scan(text).links)
}

/// Lo mismo, con los links de `text` ya escaneados.
pub(crate) fn links_of(text: &str, scanned: &[Link]) -> Vec<IndexedLink> {
    let mut lines = Lines::new(text);
    scanned
        .iter()
        .map(|link| {
            let (number, line) = lines.at(link.span.start);
            indexed(text, link, number, &line)
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
        embed: link.embed,
        span: link.span.clone(),
        target_span: link.target.clone(),
        target_text: link.target_text(text).into_owned(),
        subpath: subpath(text, link),
        line: number,
        column: link.span.start - bounds.start,
        context: context(text, &link.span, bounds),
        written: written(text, link),
        target: LinkTarget::Unresolved,
    }
}

fn written(text: &str, link: &Link) -> Written {
    let before_target = text[link.span.start..link.target.start].trim_end();
    match link.syntax {
        LinkSyntax::Wiki => Written::Wiki,
        LinkSyntax::Markdown if before_target.ends_with('<') => Written::BracketedMarkdown,
        LinkSyntax::Markdown => Written::Markdown,
    }
}

impl IndexedLink {
    /// El mismo link si su texto es parte de otro, donde empieza en el byte
    /// `start` de la línea `line`.
    pub(crate) fn shifted(mut self, start: usize, line: usize) -> Self {
        self.span = self.span.start + start..self.span.end + start;
        self.target_span = self.target_span.start + start..self.target_span.end + start;
        self.line += line;
        self
    }

    /// El link de una tarjeta de pizarra a `file`, que ocupa su línea, la
    /// `line`, desde el byte `start`. Su contexto es la ruta y el texto de
    /// los nodos `joined` a la tarjeta (09 §5).
    pub(crate) fn card<'j>(
        file: &str,
        subpath: Option<&str>,
        start: usize,
        line: usize,
        joined: impl Iterator<Item = &'j str>,
    ) -> Self {
        let span = start..start + file.len();
        Self {
            embed: false,
            span: span.clone(),
            target_span: span,
            target_text: format!("/{file}"),
            subpath: subpath.and_then(card_subpath),
            line,
            column: 0,
            context: card_context(file, joined),
            written: Written::Card,
            target: LinkTarget::Unresolved,
        }
    }
}

/// El `subpath` de una tarjeta: `#Sección` o `#^bloque`, como en JSON Canvas.
fn card_subpath(subpath: &str) -> Option<LinkSubpath> {
    let name = subpath.strip_prefix('#').unwrap_or(subpath);
    match name.strip_prefix('^') {
        _ if name.is_empty() => None,
        Some(block) => Some(LinkSubpath::Block(block.to_owned())),
        None => Some(LinkSubpath::Heading(name.to_owned())),
    }
}

fn card_context<'j>(file: &str, joined: impl Iterator<Item = &'j str>) -> LinkContext {
    let mut text = file.to_owned();
    let first_lines =
        joined.filter_map(|words| words.lines().map(str::trim).find(|l| !l.is_empty()));
    for (at, line) in first_lines.enumerate() {
        text.push_str(if at == 0 { " — " } else { " · " });
        text.push_str(line);
    }
    text.truncate(ceil_boundary(
        &text,
        (file.len() + CONTEXT_BYTES).min(text.len()),
    ));
    LinkContext {
        text,
        link: 0..file.len(),
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
        assert_eq!(links[0].written, Written::BracketedMarkdown);
        assert_eq!(links[1].target_text, "Mi Nota.md");
        assert_eq!(links[1].written, Written::Markdown);
        assert_eq!(
            links[1].subpath,
            Some(LinkSubpath::Heading("Sec 1".to_owned()))
        );
        assert_eq!(links[2].subpath, Some(LinkSubpath::Block("id".to_owned())));
    }
}
