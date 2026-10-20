//! Qué líneas de un panel muestran su fuente en vez de sus fórmulas: la
//! del cursor, las de una selección de Visual y, si el cursor está dentro
//! de un bloque `$$` de varias líneas, todas las del bloque (08 §7, como el
//! plan del live preview en 03 §2). El motor no se entera: el texto del
//! buffer es siempre la fuente, y el cursor nunca está sobre una fórmula.

use std::ops::RangeInclusive;

use metalmind_vim::Mode;

use super::PaneView;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceLines {
    /// La línea del cursor o las de la selección de Visual, con los bloques
    /// `$$` de sus extremos enteros: siempre seguidas.
    lines: RangeInclusive<usize>,
}

impl SourceLines {
    #[must_use]
    pub fn contains(&self, line: usize) -> bool {
        self.lines.contains(&line)
    }
}

impl PaneView<'_> {
    #[must_use]
    pub fn source_lines(&self) -> SourceLines {
        let buffer = self.buffer();
        let cursor = buffer.line_of(self.cursor());
        let anchor = if matches!(self.mode, Mode::Visual(_)) {
            buffer.line_of(self.selection.anchor)
        } else {
            cursor
        };
        let (mut first, mut last) = (anchor.min(cursor), anchor.max(cursor));
        if let Some(styles) = self.document.state().styles() {
            for end in [anchor, cursor] {
                if let Some(block) = styles.math_block(end) {
                    first = first.min(*block.start());
                    last = last.max(*block.end());
                }
            }
        }
        SourceLines {
            lines: first..=last,
        }
    }
}
