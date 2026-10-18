use crate::code::{self, CodeSpans};
use crate::markdown_link::{self, Brackets};
use crate::{Scan, frontmatter, link, tag};

/// Recorre la nota de izquierda a derecha. Todo disparador es ASCII, así que
/// avanzar de a un byte nunca deja un rango en medio de un carácter.
struct Scanner<'t> {
    text: &'t str,
    /// Dónde empieza el cuerpo: tras el frontmatter o el BOM.
    body: usize,
    scan: Scan,
    code_spans: CodeSpans,
    brackets: Brackets,
}

pub(crate) fn scan(text: &str) -> Scan {
    let mut scan = Scan::default();
    let (frontmatter, body) = frontmatter::parse(text, &mut scan.problems);
    scan.frontmatter = frontmatter;
    let mut scanner = Scanner {
        text,
        body,
        scan,
        code_spans: CodeSpans::default(),
        brackets: Brackets::default(),
    };
    scanner.run();
    scanner.scan
}

impl Scanner<'_> {
    fn run(&mut self) {
        let mut at = self.body;
        while at < self.text.len() {
            let line_start = at == self.body || self.text.as_bytes()[at - 1] == b'\n';
            let fence_end = if line_start {
                code::fenced_block_end(self.text, at)
            } else {
                None
            };
            at = fence_end.unwrap_or_else(|| self.step(at));
        }
    }

    /// Reconoce lo que empiece en `at` y devuelve dónde seguir.
    fn step(&mut self, at: usize) -> usize {
        let bytes = self.text.as_bytes();
        let next = bytes.get(at + 1).copied();
        match bytes[at] {
            b'\\' if next.is_some_and(|b| b.is_ascii_punctuation()) => at + 2,
            b'`' => self.code_spans.end(self.text, at),
            b'%' if next == Some(b'%') => code::comment_end(self.text, at),
            b'!' if next == Some(b'[') => self.link(at, at + 1).unwrap_or(at + 1),
            b'[' => self.link(at, at).unwrap_or(at + 1),
            b'#' if self.after_space(at) => self.tag(at),
            _ => at + 1,
        }
    }

    fn link(&mut self, start: usize, open: usize) -> Option<usize> {
        if self.text.as_bytes().get(open + 1) == Some(&b'[')
            && let Some(link) = link::wikilink_at(self.text, start, open)
        {
            let end = link.span.end;
            self.scan.links.push(link);
            return Some(end);
        }
        let markdown = markdown_link::markdown_link_at(
            self.text,
            &mut self.brackets,
            &mut self.code_spans,
            start,
            open,
        )?;
        self.scan.links.extend(markdown.link);
        Some(markdown.end)
    }

    /// Un `#` solo abre un tag al inicio de línea o tras un espacio: así no
    /// son tags el fragmento de una URL ni `texto#algo`.
    fn after_space(&self, at: usize) -> bool {
        at == self.body
            || self.text[..at]
                .chars()
                .next_back()
                .is_none_or(char::is_whitespace)
    }

    fn tag(&mut self, at: usize) -> usize {
        match tag::tag_at(self.text, at) {
            Some(tag) => {
                let end = tag.span.end;
                self.scan.tags.push(tag);
                end
            }
            None => at + 1,
        }
    }
}
