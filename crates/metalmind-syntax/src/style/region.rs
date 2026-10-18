//! Un tramo de líneas que se puede pintar sin mirar las de antes: empieza
//! en un estado conocido ([`Start`]) y termina donde el siguiente vuelve a
//! tener uno. El scanner corre igual que sobre la nota entera, porque nada
//! de lo que mira cruza el borde de un tramo: el código inline no pasa de
//! una línea en blanco ni de una cerca, y un comentario que sigue en otra
//! línea es un estado.

use super::fence::{Lang, OpenFence, highlight};
use super::inline::{Entry, style_chunk};
use super::{Conceal, FencePart, LineKind, LineStyle, Lines, Role, Span, Style};
use crate::code::{self, closes, fence, opening_fence};
use crate::scanner::RegionEnd;

/// El estado al empezar una línea.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Start {
    /// La primera línea de la nota: puede abrir el frontmatter.
    Document,
    /// En el cuerpo, fuera de todo lo que cruza líneas.
    Body,
    /// Dentro de un comentario `%%`.
    Comment,
    Fence(OpenFence),
}

pub(crate) struct Region {
    pub lines: Vec<LineStyle>,
    /// El estado de la línea que sigue al tramo.
    pub next: Start,
    /// Si ningún tramo hasta este mira la línea que sigue: una cerca o el
    /// frontmatter no miran más allá de su línea.
    pub next_hard: bool,
}

/// El tramo que empieza en la línea `first` con el estado `start`. `close`
/// es la línea que cierra el frontmatter, si lo hay.
pub(crate) fn style(
    source: &impl Lines,
    first: usize,
    start: Start,
    close: Option<usize>,
) -> Region {
    match start {
        Start::Document => document(source, close),
        Start::Body => body(
            source,
            first,
            Entry {
                body: 0,
                starts_line: true,
            },
        ),
        Start::Comment => comment(source, first),
        Start::Fence(open) => fenced(&source.line(first), open),
    }
}

fn document(source: &impl Lines, close: Option<usize>) -> Region {
    let first = source.line(0);
    let bom = if first.starts_with('\u{FEFF}') {
        '\u{FEFF}'.len_utf8()
    } else {
        0
    };
    if let Some(close) = close
        && let Some(lines) = super::frontmatter::style(&join(source, 0..close + 1))
    {
        return Region {
            lines,
            next: Start::Body,
            next_hard: true,
        };
    }
    body(
        source,
        0,
        Entry {
            body: bom,
            starts_line: true,
        },
    )
}

/// Si la primera línea abre un frontmatter (falta que algo lo cierre).
pub(crate) fn opens_frontmatter(first: &str) -> bool {
    is_delimiter(first.trim_start_matches('\u{FEFF}'))
}

pub(crate) fn is_delimiter(line: &str) -> bool {
    line.trim_end() == "---"
}

fn body(source: &impl Lines, first: usize, entry: Entry) -> Region {
    let line = source.line(first);
    let after = &line[entry.body..];
    if entry.starts_line
        && let Some((fence, info)) = opening_fence(after).and_then(|_| fence(after))
    {
        let open = OpenFence {
            fence,
            lang: Lang::of(info),
        };
        return Region {
            lines: vec![fence_line(&line, entry.body, FencePart::Open)],
            next: Start::Fence(open),
            next_hard: true,
        };
    }
    let (text, count) = paragraph(source, first, &line);
    let chunk = style_chunk(&text, entry, count);
    let next = match chunk.end {
        RegionEnd::InComment { .. } => Start::Comment,
        RegionEnd::End => Start::Body,
    };
    Region {
        lines: chunk.lines,
        next,
        next_hard: false,
    }
}

fn comment(source: &impl Lines, first: usize) -> Region {
    let line = source.line(first);
    match line.find("%%") {
        Some(at) => body(
            source,
            first,
            Entry {
                body: at + 2,
                starts_line: false,
            },
        ),
        None => Region {
            lines: vec![whole(&line, Role::Comment, LineKind::Text)],
            next: Start::Comment,
            next_hard: false,
        },
    }
}

/// El texto desde la línea `first` (que ya se leyó: `line`) hasta antes de
/// la primera que está en blanco o abre una cerca, que es hasta donde mira
/// el código inline, y cuántas líneas son. Si sigue otra línea, termina
/// con su salto, como en la nota.
fn paragraph(source: &impl Lines, first: usize, line: &str) -> (String, usize) {
    let mut text = line.to_owned();
    let mut count = 1;
    for index in first + 1..source.line_count() {
        text.push('\n');
        let next = source.line(index);
        if code::is_blank(&next) || opening_fence(&next).is_some() {
            return (text, count);
        }
        text.push_str(&next);
        count += 1;
    }
    (text, count)
}

/// Las líneas unidas por `\n`.
fn join(source: &impl Lines, lines: std::ops::Range<usize>) -> String {
    let mut text = String::new();
    for (nth, index) in lines.enumerate() {
        if nth > 0 {
            text.push('\n');
        }
        text.push_str(&source.line(index));
    }
    text
}

fn fenced(line: &str, open: OpenFence) -> Region {
    if closes(&open.fence, line) {
        return Region {
            lines: vec![fence_line(line, 0, FencePart::Close)],
            next: Start::Body,
            next_hard: true,
        };
    }
    let mut style = whole(line, Role::Code, LineKind::Fence(FencePart::Body));
    if let Some(lang) = open.lang {
        style.spans = code_spans(line, lang);
    }
    Region {
        lines: vec![style],
        next: Start::Fence(open),
        next_hard: true,
    }
}

/// El código de la línea con sus tokens encima, sin huecos.
fn code_spans(line: &str, lang: Lang) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut at = 0;
    let plain = |range: std::ops::Range<usize>| Span {
        range,
        style: Style {
            role: Role::Code,
            ..Style::default()
        },
        conceal: None,
        link: None,
    };
    for (range, role) in highlight(line, lang) {
        if range.start > at {
            spans.push(plain(at..range.start));
        }
        at = range.end;
        spans.push(Span {
            style: Style {
                role,
                ..Style::default()
            },
            ..plain(range)
        });
    }
    if at < line.len() {
        spans.push(plain(at..line.len()));
    }
    spans
}

fn whole(line: &str, role: Role, kind: LineKind) -> LineStyle {
    let spans = if line.is_empty() {
        Vec::new()
    } else {
        vec![Span {
            range: 0..line.len(),
            style: Style {
                role,
                ..Style::default()
            },
            conceal: None,
            link: None,
        }]
    };
    LineStyle {
        kind,
        spans,
        ..LineStyle::default()
    }
}

/// Una cerca desde `from`: lo de antes de la racha (sangría, `>`, un
/// marcador de lista) queda como texto, la racha se oculta y el lenguaje
/// se ve aparte.
fn fence_line(line: &str, from: usize, part: FencePart) -> LineStyle {
    let mut spans = Vec::new();
    if let Some((found, rest)) = fence(&line[from..]) {
        let run_end = line.len() - rest.len();
        spans.push(Span {
            range: run_end - found.len..run_end,
            style: Style {
                role: Role::Markup,
                ..Style::default()
            },
            conceal: Some(Conceal::Hide),
            link: None,
        });
        let info = rest.trim();
        if !info.is_empty() {
            let start = run_end + (rest.len() - rest.trim_start().len());
            spans.push(Span {
                range: start..start + info.len(),
                style: Style {
                    role: Role::FenceInfo,
                    ..Style::default()
                },
                conceal: None,
                link: None,
            });
        }
    }
    LineStyle {
        kind: LineKind::Fence(part),
        spans,
        ..LineStyle::default()
    }
}
