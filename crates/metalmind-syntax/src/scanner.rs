use crate::code::{self, CodeSpans};
use crate::markdown_link::{self, Brackets};
use crate::sink::Sink;
use crate::{Scan, anchor, frontmatter, link, tag};

/// Recorre la nota de izquierda a derecha. Todo disparador es ASCII, así que
/// avanzar de a un byte nunca deja un rango en medio de un carácter.
pub(crate) struct Scanner<'t, S> {
    text: &'t str,
    /// Dónde empieza el cuerpo: tras el frontmatter o el BOM.
    body: usize,
    /// Si `body` es el inicio de una línea. No lo es en un tramo que sigue
    /// al cierre de un comentario en medio de la línea.
    body_starts_line: bool,
    sink: S,
    code_spans: CodeSpans,
    brackets: Brackets,
}

pub(crate) fn scan(text: &str) -> Scan {
    let mut scan = Scan::default();
    let (frontmatter, body) = frontmatter::parse(text, &mut scan.problems);
    scan.frontmatter = frontmatter;
    let mut scanner = Scanner::new(text, body, true, scan);
    scanner.run();
    scanner.sink
}

/// Dónde se detuvo [`Scanner::run_region`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RegionEnd {
    /// Llegó al final del tramo.
    End,
    /// Un comentario sigue tras la línea: su tramo llega hasta `line_end`.
    InComment { line_end: usize },
}

impl<'t, S: Sink> Scanner<'t, S> {
    pub(crate) fn new(text: &'t str, body: usize, body_starts_line: bool, sink: S) -> Self {
        Self {
            text,
            body,
            body_starts_line,
            sink,
            code_spans: CodeSpans::default(),
            brackets: Brackets::default(),
        }
    }

    pub(crate) fn into_sink(self) -> S {
        self.sink
    }

    fn run(&mut self) {
        let mut at = self.body;
        while at < self.text.len() {
            let fence_end = if self.line_start(at) {
                code::fenced_block_end(self.text, at)
            } else {
                None
            };
            at = fence_end.unwrap_or_else(|| self.step(at));
        }
    }

    /// Recorre un tramo sin cercas desde `body`, como lo recorrería
    /// [`scan`], salvo que un comentario que sigue en otra línea corta el
    /// tramo al final de la suya: quien llama sigue desde ahí línea a línea.
    pub(crate) fn run_region(&mut self) -> RegionEnd {
        let mut at = self.body;
        while at < self.text.len() {
            if self.text.as_bytes()[at..].starts_with(b"%%") {
                let line_end = code::line_end(self.text, at);
                let Some(i) = self.text[at + 2..line_end].find("%%") else {
                    self.sink.comment(at..line_end);
                    return RegionEnd::InComment { line_end };
                };
                let end = at + 2 + i + 2;
                self.sink.comment(at..end);
                at = end;
            } else {
                at = self.step(at);
            }
        }
        RegionEnd::End
    }

    fn line_start(&self, at: usize) -> bool {
        if at == self.body {
            self.body_starts_line
        } else {
            at > self.body && self.text.as_bytes()[at - 1] == b'\n'
        }
    }

    /// Reconoce lo que empiece en `at` y devuelve dónde seguir.
    fn step(&mut self, at: usize) -> usize {
        let bytes = self.text.as_bytes();
        let next = bytes.get(at + 1).copied();
        match bytes[at] {
            b'\\' if next.is_some_and(|b| b.is_ascii_punctuation()) => {
                self.sink.escape(at);
                at + 2
            }
            b'`' => self.code_span(at),
            b'%' if next == Some(b'%') => {
                let end = code::comment_end(self.text, at);
                self.sink.comment(at..end);
                end
            }
            b'!' if next == Some(b'[') => self.link(at, at + 1).unwrap_or(at + 1),
            b'[' => self.link(at, at).unwrap_or(at + 1),
            b'#' | b'^' if self.after_space(at) => self.after_space_trigger(at),
            _ => at + 1,
        }
    }

    fn code_span(&mut self, at: usize) -> usize {
        let (end, closed) = self.code_spans.find(self.text, at);
        if closed {
            self.sink.code_span(at..end);
        }
        end
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
            self.sink.link(link);
            return Some(end);
        }
        let markdown = markdown_link::markdown_link_at(
            self.text,
            &mut self.brackets,
            &mut self.code_spans,
            start,
            open,
        )?;
        let end = markdown.span.end;
        self.sink.markdown_link(markdown);
        Some(end)
    }

    /// Un `#` o un `^` solo abren algo al inicio de línea o tras un espacio:
    /// así no son tags el fragmento de una URL ni `texto#algo`.
    fn after_space(&self, at: usize) -> bool {
        if at == self.body {
            return self.body_starts_line;
        }
        let before = self.text.as_bytes()[at - 1];
        before.is_ascii_whitespace()
            || ((!before.is_ascii() || before == 0x0b) && self.after_other_space(at))
    }

    /// Lo que `is_ascii_whitespace` no cubre: el tab vertical y los
    /// espacios de Unicode. Aparte para que `after_space`, que corre en cada
    /// `#`, se siga inlineando.
    #[cold]
    fn after_other_space(&self, at: usize) -> bool {
        self.text[self.body..at]
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
        if indent <= 3
            && self.line_start(line_start)
            && let Some(heading) = anchor::heading_at(self.text, line_start)
        {
            self.sink.heading(heading);
        }
    }

    fn block(&mut self, at: usize) -> usize {
        match anchor::block_at(self.text, at) {
            Some(block) => {
                let end = block.span.end;
                self.sink.block(block);
                end
            }
            None => at + 1,
        }
    }

    fn tag(&mut self, at: usize) -> usize {
        match tag::tag_at(self.text, at) {
            Some(tag) => {
                let end = tag.span.end;
                self.sink.tag(tag);
                end
            }
            None => at + 1,
        }
    }
}

/// Recorre `text`, un tramo sin cercas, desde `from`; `starts_line` dice si
/// `from` es el inicio de una línea.
pub(crate) fn region<S: Sink>(
    text: &str,
    from: usize,
    starts_line: bool,
    sink: S,
) -> (S, RegionEnd) {
    let mut scanner = Scanner::new(text, from, starts_line, sink);
    let end = scanner.run_region();
    (scanner.into_sink(), end)
}
