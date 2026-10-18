use crate::code::{self, CodeSpans};
use crate::markdown_link::{self, Brackets};
use crate::{Scan, anchor, frontmatter, link, tag};

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
            b'#' | b'^' if self.after_space(at) => self.after_space_trigger(at),
            _ => at + 1,
        }
    }

    /// Un `#` (heading o tag) o un `^` (bloque) que empieza tras un espacio.
    fn after_space_trigger(&mut self, at: usize) -> usize {
        if self.text.as_bytes()[at] == b'^' {
            return self.block(at);
        }
        self.heading(at);
        self.tag(at)
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

    /// Un `#` o un `^` solo abren algo al inicio de línea o tras un espacio:
    /// así no son tags el fragmento de una URL ni `texto#algo`.
    fn after_space(&self, at: usize) -> bool {
        at == self.body || {
            let before = self.text.as_bytes()[at - 1];
            before.is_ascii_whitespace()
                || ((!before.is_ascii() || before == 0x0b) && self.after_other_space(at))
        }
    }

    /// Lo que `is_ascii_whitespace` no cubre: el tab vertical y los
    /// espacios de Unicode. Aparte para que `after_space`, que corre en cada
    /// `#`, se siga inlineando.
    #[cold]
    fn after_other_space(&self, at: usize) -> bool {
        self.text[..at]
            .chars()
            .next_back()
            .is_some_and(char::is_whitespace)
    }

    /// Un heading empieza con el primer `#` de la línea, tras hasta tres
    /// espacios.
    fn heading(&mut self, at: usize) {
        let indent = self.text.as_bytes()[self.body..at]
            .iter()
            .rev()
            .take(4)
            .take_while(|&&b| b == b' ')
            .count();
        let line_start = at - indent;
        let at_line_start =
            line_start == self.body || self.text.as_bytes()[line_start - 1] == b'\n';
        if indent <= 3
            && at_line_start
            && let Some(heading) = anchor::heading_at(self.text, line_start)
        {
            self.scan.headings.push(heading);
        }
    }

    fn block(&mut self, at: usize) -> usize {
        match anchor::block_at(self.text, at) {
            Some(block) => {
                let end = block.span.end;
                self.scan.blocks.push(block);
                end
            }
            None => at + 1,
        }
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
