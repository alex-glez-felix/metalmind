//! Lo que el scanner encontró en un tramo, todo lo que hace falta para
//! pintarlo.

use std::ops::Range;

use crate::markdown_link::MarkdownLink;
use crate::math::Math;
use crate::sink::Sink;
use crate::{BlockId, Heading, Link, Tag};

#[derive(Default)]
pub(crate) struct Pieces {
    pub wikilinks: Vec<Link>,
    pub markdown: Vec<MarkdownLink>,
    pub tags: Vec<Tag>,
    pub headings: Vec<Heading>,
    pub blocks: Vec<BlockId>,
    pub code_spans: Vec<Range<usize>>,
    pub comments: Vec<Range<usize>>,
    pub escapes: Vec<usize>,
    pub math: Vec<Math>,
}

impl Sink for Pieces {
    fn link(&mut self, link: Link) {
        self.wikilinks.push(link);
    }

    fn markdown_link(&mut self, markdown: MarkdownLink) {
        self.markdown.push(markdown);
    }

    fn tag(&mut self, tag: Tag) {
        self.tags.push(tag);
    }

    fn heading(&mut self, heading: Heading) {
        self.headings.push(heading);
    }

    fn block(&mut self, block: BlockId) {
        self.blocks.push(block);
    }

    fn code_span(&mut self, span: Range<usize>) {
        self.code_spans.push(span);
    }

    fn comment(&mut self, span: Range<usize>) {
        self.comments.push(span);
    }

    fn escape(&mut self, at: usize) {
        self.escapes.push(at);
    }

    fn math(&mut self, math: Math) {
        self.math.push(math);
    }
}

impl Pieces {
    /// Lo que no es texto para la negrita ni para un marcador de bloque:
    /// código, comentarios, fórmulas, links (salvo el texto de uno
    /// markdown), tags, escapes y `^bloques`. Ordenado y sin solaparse.
    pub(crate) fn opaque(&self) -> Vec<Range<usize>> {
        let mut ranges: Vec<Range<usize>> = self
            .code_spans
            .iter()
            .chain(&self.comments)
            .cloned()
            .chain(self.math.iter().map(|math| math.span.clone()))
            .chain(self.wikilinks.iter().map(|link| link.span.clone()))
            .chain(self.markdown.iter().flat_map(|link| {
                [
                    link.span.start..link.label.start,
                    link.label.end..link.span.end,
                ]
            }))
            .chain(self.tags.iter().map(|tag| tag.span.clone()))
            .chain(self.escapes.iter().map(|&at| at..at + 2))
            .chain(self.blocks.iter().map(|block| block.span.clone()))
            .filter(|range| !range.is_empty())
            .collect();
        ranges.sort_by_key(|range| range.start);
        let mut merged: Vec<Range<usize>> = Vec::with_capacity(ranges.len());
        for range in ranges {
            match merged.last_mut() {
                Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
                _ => merged.push(range),
            }
        }
        merged
    }
}
