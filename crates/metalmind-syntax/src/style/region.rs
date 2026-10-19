//! Un tramo de líneas que se puede pintar sin mirar las de antes: empieza
//! en un estado conocido ([`Start`]) y termina donde el siguiente vuelve a
//! tener uno. El scanner corre igual que sobre la nota entera, porque nada
//! de lo que mira cruza el borde de un tramo: el código inline no pasa de
//! una línea en blanco ni de una cerca, y un comentario que sigue en otra
//! línea es un estado.

use std::hash::{DefaultHasher, Hash as _, Hasher as _};

use super::fence::{Lang, OpenFence, highlight};
use super::inline::{BlockEnd, Entry, Lead, style_chunk};
pub(crate) use super::paragraph::Paragraph;
use super::{Conceal, FencePart, LineKind, LineStyle, Lines, Role, Span, Style};
use crate::code::{self, closes, fence, opening_fence};
use crate::math;
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
    /// Dentro de un bloque `$$`.
    Math(OpenMath),
}

/// Un bloque `$$` que sigue abierto. Se cuenta desde la línea que lo abrió
/// y no con su número, así una línea insertada arriba no cambia el estado
/// de las de abajo. La línea que lo cierra lleva el LaTeX de todas: el
/// estado resume lo que lleva escrito, para que cambiar una línea del
/// bloque no deje de repintar el cierre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OpenMath {
    /// Cuántas líneas arriba abrió.
    pub opened: usize,
    /// Dónde empieza el LaTeX en la línea que lo abrió.
    pub column: usize,
    /// Un hash del LaTeX hasta la línea de antes.
    pub digest: u64,
}

impl OpenMath {
    fn new(column: usize, first: &str) -> Self {
        Self {
            opened: 1,
            column,
            digest: fold(0, first),
        }
    }

    fn next(self, line: &str) -> Self {
        Self {
            opened: self.opened + 1,
            digest: fold(self.digest, line),
            ..self
        }
    }
}

fn fold(digest: u64, line: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    digest.hash(&mut hasher);
    line.hash(&mut hasher);
    hasher.finish()
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
/// es la línea que cierra el frontmatter, si lo hay. `paragraph` es el
/// último párrafo leído del mismo texto: si `first` es una de sus líneas,
/// no se vuelve a leer.
pub(crate) fn style(
    source: &impl Lines,
    first: usize,
    start: Start,
    close: Option<usize>,
    paragraph: &mut Option<Paragraph>,
) -> Region {
    let mut read = Reader { source, paragraph };
    match start {
        Start::Document => document(&mut read, close),
        Start::Body => body(&mut read, first, Entry::line(0)),
        Start::Comment => comment(&mut read, first),
        Start::Fence(open) => fenced(&source.line(first), open),
        Start::Math(open) => math_block(&mut read, first, open),
    }
}

/// Las líneas de la nota y el párrafo que se está recorriendo.
struct Reader<'a, L> {
    source: &'a L,
    paragraph: &'a mut Option<Paragraph>,
}

fn document<L: Lines>(read: &mut Reader<'_, L>, close: Option<usize>) -> Region {
    let source = read.source;
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
    body(read, 0, Entry::line(bom))
}

/// Si la primera línea abre un frontmatter (falta que algo lo cierre).
pub(crate) fn opens_frontmatter(first: &str) -> bool {
    is_delimiter(first.trim_start_matches('\u{FEFF}'))
}

pub(crate) fn is_delimiter(line: &str) -> bool {
    line.trim_end() == "---"
}

fn body<L: Lines>(read: &mut Reader<'_, L>, first: usize, entry: Entry) -> Region {
    let line = read.source.line(first);
    let after = &line[entry.body..];
    if entry.starts_line()
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
    if read
        .paragraph
        .as_ref()
        .is_some_and(|paragraph| paragraph.index_of(first).is_none())
    {
        *read.paragraph = None;
    }
    let paragraph = read
        .paragraph
        .get_or_insert_with(|| Paragraph::read(read.source, first, &line));
    let chunk = style_chunk(paragraph, first, entry);
    let next = match chunk.end {
        RegionEnd::InComment { .. } => Start::Comment,
        RegionEnd::InMath { line_end, column } => {
            let text = &paragraph.text;
            let start = code::line_start(text, line_end);
            let first = text.get(start + column..line_end).unwrap_or_default();
            Start::Math(OpenMath::new(column, first))
        }
        RegionEnd::End => Start::Body,
    };
    Region {
        lines: chunk.lines,
        next,
        next_hard: false,
    }
}

fn comment<L: Lines>(read: &mut Reader<'_, L>, first: usize) -> Region {
    let line = read.source.line(first);
    match line.find("%%") {
        Some(at) => body(
            read,
            first,
            Entry {
                body: at + 2,
                lead: Lead::Comment,
            },
        ),
        None => Region {
            lines: vec![whole(&line, Role::Comment, LineKind::Text)],
            next: Start::Comment,
            next_hard: false,
        },
    }
}

/// Una línea dentro de un bloque `$$`: si lo cierra, lo que sigue al `$$`
/// es cuerpo y la línea lleva la fórmula entera.
fn math_block<L: Lines>(read: &mut Reader<'_, L>, first: usize, open: OpenMath) -> Region {
    let line = read.source.line(first);
    let Some(close) = math::display_close(&line, 0, line.len()) else {
        return Region {
            lines: vec![whole(&line, Role::Math, LineKind::Text)],
            next: Start::Math(open.next(&line)),
            next_hard: false,
        };
    };
    let block = BlockEnd {
        latex: block_latex(read.source, first, open, &line[..close]),
        above: open.opened,
    };
    body(
        read,
        first,
        Entry {
            body: close + 2,
            lead: Lead::Math(block),
        },
    )
}

/// El LaTeX de un bloque que cierra en la línea `last`, donde termina
/// con `tail`. Las líneas de arriba no cambiaron desde que se abrió: si
/// cambiaran, el tramo empezaría antes.
fn block_latex(source: &impl Lines, last: usize, open: OpenMath, tail: &str) -> String {
    let first = last.saturating_sub(open.opened);
    let mut latex = String::new();
    for index in first..last {
        let line = source.line(index);
        let part = if index == first {
            line.get(open.column..).unwrap_or_default()
        } else {
            &line
        };
        latex.push_str(part);
        latex.push('\n');
    }
    latex.push_str(tail);
    latex
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
