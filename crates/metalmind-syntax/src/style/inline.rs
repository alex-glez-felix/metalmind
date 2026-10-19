//! El estilo de un tramo del cuerpo: lo que encontró el scanner, los
//! marcadores de bloque de cada línea y la negrita, pasados a capas y de
//! ahí a los tramos de cada línea.

use std::ops::Range;

use super::block::{BlockKind, BlockLine, block_line};
use super::emphasis::{Delimiters, Styled};
use super::flatten::{Layer, Layers};
use super::pieces::Pieces;
use super::{Conceal, LineKind, LineStyle, LinkSpot, MathSpot, Role};
use crate::markdown_link::MarkdownLink;
use crate::math::Math;
use crate::scanner::{self, RegionEnd};
use crate::{Heading, Link};

/// Las líneas de un tramo ya pintadas y dónde se detuvo el scanner.
pub(crate) struct Chunk {
    pub lines: Vec<LineStyle>,
    pub end: RegionEnd,
}

/// Cómo empieza el tramo: en `body` de su primera línea, tras `lead`.
#[derive(Debug, Clone)]
pub(crate) struct Entry {
    pub body: usize,
    pub lead: Lead,
}

impl Entry {
    /// Desde el inicio de la línea, o tras su BOM.
    pub(crate) fn line(body: usize) -> Self {
        Self {
            body,
            lead: Lead::Line,
        }
    }

    pub(crate) fn starts_line(&self) -> bool {
        matches!(self.lead, Lead::Line)
    }
}

/// Lo que hay antes de `body` en la primera línea del tramo.
#[derive(Debug, Clone)]
pub(crate) enum Lead {
    /// Nada: `body` es el inicio de la línea.
    Line,
    /// El cierre de un comentario `%%`.
    Comment,
    /// El cierre de un bloque `$$` de varias líneas, que termina en `body`.
    Math(BlockEnd),
}

/// Un bloque `$$` que cierra en esta línea.
#[derive(Debug, Clone)]
pub(crate) struct BlockEnd {
    /// El LaTeX entero, de todas sus líneas.
    pub latex: String,
    /// Cuántas líneas arriba abrió.
    pub above: usize,
}

/// Los rangos de cada línea de `text`, sin el salto.
pub(crate) fn line_ranges(text: &str) -> Vec<Range<usize>> {
    let mut lines = Vec::new();
    let mut start = 0;
    for (i, _) in text.match_indices('\n') {
        lines.push(start..i);
        start = i + 1;
    }
    lines.push(start..text.len());
    lines
}

/// Las `count` líneas de `text`, que puede terminar con el salto de la
/// última.
pub(crate) fn style_chunk(text: &str, entry: Entry, count: usize) -> Chunk {
    let starts_line = entry.starts_line();
    let (pieces, end) = scanner::region(text, entry.body, starts_line, Pieces::default());
    let mut lines = line_ranges(text);
    lines.truncate(count);
    if let RegionEnd::InComment { line_end } | RegionEnd::InMath { line_end, .. } = end {
        lines.retain(|line| line.start <= line_end);
    }
    let mut opaque = pieces.opaque();
    let mut layers = Layers::default();
    let mut formulas = vec![Vec::new(); lines.len()];
    match entry.lead {
        Lead::Line => {}
        Lead::Comment => layers.push(0..entry.body, Layer::Role(Role::Comment)),
        Lead::Math(block) => {
            let close = entry.body - 2;
            layers.push(0..close, Layer::Role(Role::Math));
            hide(close..entry.body, &mut layers);
            if !block.latex.trim().is_empty() {
                formulas[0].push(MathSpot {
                    range: 0..entry.body,
                    latex: block.latex,
                    display: true,
                    above: block.above,
                });
            }
        }
    }
    if !starts_line {
        opaque.insert(0, 0..entry.body);
    }
    let mut out = vec![LineStyle::default(); lines.len()];
    let painter = Painter {
        text,
        lines: &lines,
        opaque: &opaque,
    };
    let blocks = painter.blocks(
        entry.body,
        starts_line,
        &pieces.headings,
        &mut layers,
        &mut out,
    );
    painter.emphasis(&blocks, &pieces, &mut layers);
    let mut spots = vec![Vec::new(); lines.len()];
    painter.pieces(&pieces, &mut layers, &mut spots);
    painter.math(&pieces.math, &mut layers, &mut formulas);
    let flat = layers.flatten(&lines);
    for (((line, spans), spots), math) in out.iter_mut().zip(flat).zip(spots).zip(formulas) {
        line.spans = spans;
        line.links = spots;
        line.math = math;
    }
    Chunk { lines: out, end }
}

/// Lo que mira cada línea para saber en qué bloque está y dónde empieza
/// su texto.
struct LineBlock {
    /// Dónde empieza el texto, tras los marcadores.
    content: usize,
    /// Un bloque para la negrita: no se empareja de uno a otro.
    block: usize,
    rule: bool,
}

struct Painter<'a> {
    text: &'a str,
    lines: &'a [Range<usize>],
    opaque: &'a [Range<usize>],
}

impl Painter<'_> {
    fn line_of(&self, at: usize) -> usize {
        self.lines
            .partition_point(|line| line.end < at)
            .min(self.lines.len() - 1)
    }

    /// Si la línea empieza dentro de un código inline o un comentario.
    fn continues(&self, start: usize) -> bool {
        let index = self.opaque.partition_point(|range| range.end <= start);
        self.opaque
            .get(index)
            .is_some_and(|range| range.start < start)
    }

    fn blocks(
        &self,
        body: usize,
        starts_line: bool,
        headings: &[Heading],
        layers: &mut Layers,
        out: &mut [LineStyle],
    ) -> Vec<LineBlock> {
        let mut blocks: Vec<LineBlock> = Vec::with_capacity(self.lines.len());
        let mut headings = headings.iter().peekable();
        let mut previous_quote = None;
        let mut previous_alone = true;
        let mut block = 0;
        for (index, range) in self.lines.iter().enumerate() {
            let line = &self.text[range.clone()];
            let start = if index == 0 { body } else { range.start };
            let heading = headings.next_if(|heading| heading.span.start == start);
            let structured = (index > 0 || starts_line) && !self.continues(range.start);
            let parsed = if structured {
                block_line(&line[start - range.start..])
            } else {
                BlockLine::default()
            };
            let base = start;
            let quote = u8::try_from(parsed.quotes.len()).unwrap_or(u8::MAX);
            let mut content = parsed.quotes.last().map_or(base, |&at| base + at + 1);
            for &at in &parsed.quotes {
                layers.push(base + at..base + at + 1, Layer::Role(Role::Markup));
                layers.push(base + at..base + at + 1, Layer::Conceal(Conceal::QuoteBar));
            }
            if quote > 0 {
                layers.push(content..range.end, Layer::Role(Role::Quote));
            }
            let alone = heading.is_some() || !matches!(parsed.kind, BlockKind::Paragraph);
            let blank = line.trim().is_empty();
            let new_block =
                index == 0 || alone || blank || previous_alone || previous_quote != Some(quote);
            if new_block {
                block += 1;
            }
            out[index].quote = quote;
            if let Some(heading) = heading {
                out[index].kind = LineKind::Heading(heading.level);
                paint_heading(heading, layers);
                content = heading.text.start;
            }
            let rule = match parsed.kind {
                BlockKind::Rule(rule) => {
                    let rule = base + rule.start..base + rule.end;
                    layers.push(rule.clone(), Layer::Role(Role::Markup));
                    layers.push(rule, Layer::Conceal(Conceal::Rule));
                    out[index].kind = LineKind::Rule;
                    true
                }
                BlockKind::Item(item) => {
                    let marker = base + item.marker.start..base + item.marker.end;
                    layers.push(marker.clone(), Layer::Role(Role::ListMarker));
                    if item.bullet {
                        layers.push(marker.clone(), Layer::Conceal(Conceal::Bullet));
                    }
                    content = marker.end;
                    if let Some(task) = item.task {
                        let task_range = base + task.range.start..base + task.range.end;
                        layers.push(task_range.clone(), Layer::Role(Role::Checkbox));
                        let checked = task.checked;
                        layers.push(
                            task_range.clone(),
                            Layer::Conceal(Conceal::Checkbox { checked }),
                        );
                        if checked {
                            layers.push(task_range.end..range.end, Layer::Role(Role::TaskDone));
                        }
                        content = task_range.end;
                    }
                    false
                }
                BlockKind::Paragraph => false,
            };
            previous_quote = Some(quote);
            previous_alone = heading.is_some() || rule || blank;
            blocks.push(LineBlock {
                content,
                block,
                rule,
            });
        }
        blocks
    }

    fn emphasis(&self, blocks: &[LineBlock], pieces: &Pieces, layers: &mut Layers) {
        let labels: Vec<Range<usize>> = pieces
            .markdown
            .iter()
            .map(|link| link.label.clone())
            .collect();
        let groups = blocks.last().map_or(0, |line| line.block + 1);
        let mut outer: Vec<Delimiters> = (0..groups).map(|_| Delimiters::new()).collect();
        let mut inner: Vec<Delimiters> = labels.iter().map(|_| Delimiters::new()).collect();
        let mut next_opaque = 0;
        for (range, line) in self.lines.iter().zip(blocks) {
            if line.rule {
                continue;
            }
            let mut at = line.content;
            while at < range.end {
                while self.opaque.get(next_opaque).is_some_and(|r| r.end <= at) {
                    next_opaque += 1;
                }
                let limit = match self.opaque.get(next_opaque) {
                    Some(r) if r.start <= at => {
                        at = r.end;
                        continue;
                    }
                    Some(r) => r.start.min(range.end),
                    None => range.end,
                };
                let candidate = self.text.as_bytes()[at..limit]
                    .iter()
                    .position(|&b| matches!(b, b'*' | b'_' | b'~' | b'='));
                let Some(offset) = candidate else {
                    at = limit;
                    continue;
                };
                at += offset;
                let label = labels.partition_point(|label| label.end <= at);
                let delimiters = match labels.get(label) {
                    Some(label_range) if label_range.start <= at => &mut inner[label],
                    _ => &mut outer[line.block],
                };
                let before = self.text[..at].chars().next_back();
                at = delimiters
                    .scan(self.text, at, before, limit)
                    .unwrap_or_else(|| {
                        at + self.text[at..].chars().next().map_or(1, char::len_utf8)
                    });
            }
        }
        for found in outer.into_iter().chain(inner).flat_map(Delimiters::pair) {
            let flag = match found.style {
                Styled::Strong => Layer::Strong,
                Styled::Emphasis => Layer::Emphasis,
                Styled::Strike => Layer::Strike,
                Styled::Mark => Layer::Mark,
            };
            layers.push(found.inner, flag);
            for marker in [found.open, found.close] {
                layers.push(marker.clone(), Layer::Role(Role::Markup));
                layers.push(marker, Layer::Conceal(Conceal::Hide));
            }
        }
    }

    fn pieces(&self, pieces: &Pieces, layers: &mut Layers, links: &mut [Vec<LinkSpot>]) {
        let mut wikilinks = pieces.wikilinks.iter().peekable();
        let mut markdown = pieces.markdown.iter().peekable();
        // En orden, para que los links de cada línea vayan como en el texto.
        loop {
            let wiki_first = match (wikilinks.peek(), markdown.peek()) {
                (Some(wiki), Some(md)) => wiki.span.start < md.span.start,
                (Some(_), None) => true,
                (None, Some(_)) => false,
                (None, None) => break,
            };
            if wiki_first {
                if let Some(link) = wikilinks.next() {
                    let index = self.spot(link, links);
                    paint_wikilink(link, index, layers);
                }
            } else if let Some(md) = markdown.next() {
                self.markdown_link(md, layers, links);
            }
        }
        for tag in &pieces.tags {
            layers.push(tag.span.clone(), Layer::Role(Role::Tag));
        }
        for span in &pieces.code_spans {
            let run = self.text.as_bytes()[span.clone()]
                .iter()
                .take_while(|&&b| b == b'`')
                .count();
            hide(span.start..span.start + run, layers);
            hide(span.end - run..span.end, layers);
            layers.push(span.start + run..span.end - run, Layer::Role(Role::Code));
            layers.push(span.start + run..span.end - run, Layer::Code);
        }
        for comment in &pieces.comments {
            layers.push(comment.clone(), Layer::Role(Role::Comment));
        }
        for &at in &pieces.escapes {
            hide(at..at + 1, layers);
        }
        for block in &pieces.blocks {
            layers.push(block.span.clone(), Layer::Role(Role::BlockId));
        }
    }

    fn markdown_link(
        &self,
        markdown: &MarkdownLink,
        layers: &mut Layers,
        links: &mut [Vec<LinkSpot>],
    ) {
        hide(markdown.span.start..markdown.label.start, layers);
        hide(markdown.label.end..markdown.span.end, layers);
        let embed = self.text.as_bytes()[markdown.span.start] == b'!';
        let role = match (&markdown.link, embed) {
            (_, true) => Role::Embed,
            (Some(_), false) => Role::Link,
            (None, false) => Role::Url,
        };
        layers.push(markdown.label.clone(), Layer::Role(role));
        if let Some(link) = &markdown.link {
            let index = self.spot(link, links);
            layers.push(link.span.clone(), Layer::Link(index));
        }
    }

    /// Anota el link en su línea y devuelve su índice ahí.
    fn spot(&self, link: &Link, links: &mut [Vec<LinkSpot>]) -> usize {
        let index = self.line_of(link.span.start);
        let start = self.lines[index].start;
        let spots = &mut links[index];
        spots.push(LinkSpot {
            range: link.span.start - start..link.span.end - start,
            target: link.target_text(self.text).into_owned(),
            embed: link.embed,
        });
        spots.len() - 1
    }
}

impl Painter<'_> {
    /// Los `$` se ocultan; el LaTeX de una fórmula cerrada se reemplaza
    /// por la fórmula, y se anota en su línea.
    fn math(&self, found: &[Math], layers: &mut Layers, formulas: &mut [Vec<MathSpot>]) {
        for math in found {
            hide(math.span.start..math.content.start, layers);
            hide(math.content.end..math.span.end, layers);
            layers.push(math.content.clone(), Layer::Role(Role::Math));
            let latex = &self.text[math.content.clone()];
            if !math.closed || latex.trim().is_empty() {
                continue;
            }
            let display = math.display;
            layers.push(
                math.content.clone(),
                Layer::Conceal(Conceal::Math { display }),
            );
            let index = self.line_of(math.span.start);
            let start = self.lines[index].start;
            formulas[index].push(MathSpot {
                range: math.span.start - start..math.span.end - start,
                latex: latex.to_owned(),
                display,
                above: 0,
            });
        }
    }
}

fn hide(range: Range<usize>, layers: &mut Layers) {
    layers.push(range.clone(), Layer::Role(Role::Markup));
    layers.push(range, Layer::Conceal(Conceal::Hide));
}

/// Los `#` y el espacio de delante se ocultan, igual que los `#` de cierre.
fn paint_heading(heading: &Heading, layers: &mut Layers) {
    let line = &heading.span;
    if heading.text.is_empty() {
        hide(line.clone(), layers);
        return;
    }
    hide(line.start..heading.text.start, layers);
    hide(heading.text.end..line.end, layers);
    layers.push(heading.text.clone(), Layer::Role(Role::Heading));
}

/// `[[` y `]]` se ocultan, y con alias también el destino: se ve el alias.
/// En un embed lo de tras la `|` es el tamaño, así que se ve todo.
fn paint_wikilink(link: &Link, index: usize, layers: &mut Layers) {
    let open_end = link.span.start + 2 + usize::from(link.embed);
    let close_start = link.span.end - 2;
    hide(link.span.start..open_end, layers);
    hide(close_start..link.span.end, layers);
    let visible = match &link.alias {
        Some(alias) if !link.embed => {
            hide(open_end..alias.start, layers);
            alias.clone()
        }
        _ => open_end..close_start,
    };
    let role = if link.embed {
        Role::Embed
    } else {
        Role::Wikilink
    };
    layers.push(visible, Layer::Role(role));
    layers.push(link.span.clone(), Layer::Link(index));
}
