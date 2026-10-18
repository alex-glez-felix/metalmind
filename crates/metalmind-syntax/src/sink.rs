//! A quién le avisa el scanner de lo que encuentra. El índice solo quiere
//! links, tags, headings y bloques; los estilos del editor quieren además
//! lo que tapa la sintaxis (código, comentarios, escapes) y los links
//! externos. Los métodos de más no hacen nada por defecto, así el scan del
//! índice no paga por ellos.

use std::ops::Range;

use crate::markdown_link::MarkdownLink;
use crate::{BlockId, Heading, Link, Scan, Tag};

pub(crate) trait Sink {
    fn link(&mut self, link: Link);
    fn tag(&mut self, tag: Tag);
    fn heading(&mut self, heading: Heading);
    fn block(&mut self, block: BlockId);

    fn markdown_link(&mut self, markdown: MarkdownLink) {
        if let Some(link) = markdown.link {
            self.link(link);
        }
    }

    /// Código inline cerrado, con sus backticks.
    fn code_span(&mut self, _span: Range<usize>) {}

    /// Un comentario `%% %%` o el tramo de una línea que cubre.
    fn comment(&mut self, _span: Range<usize>) {}

    /// La `\` de una puntuación escapada.
    fn escape(&mut self, _at: usize) {}
}

impl Sink for Scan {
    fn link(&mut self, link: Link) {
        self.links.push(link);
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
}
