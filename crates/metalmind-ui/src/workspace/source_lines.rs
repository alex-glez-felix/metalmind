//! Qué líneas de un panel muestran su fuente en vez de sus fórmulas: la
//! del cursor, las de una selección de Visual y, si el cursor está dentro
//! de un bloque `$$` de varias líneas, todas las del bloque (08 §7, como el
//! plan del live preview en 03 §2). El motor no se entera: el texto del
//! buffer es siempre la fuente, y el cursor nunca está sobre una fórmula.

use std::ops::RangeInclusive;

use metalmind_vim::{Layout, Mode};

use super::PaneView;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceLines {
    /// La línea del cursor o las de la selección de Visual.
    lines: RangeInclusive<usize>,
    /// El bloque `$$` donde está el cursor.
    block: Option<RangeInclusive<usize>>,
}

impl SourceLines {
    #[must_use]
    pub fn contains(&self, line: usize) -> bool {
        self.lines.contains(&line)
            || self
                .block
                .as_ref()
                .is_some_and(|block| block.contains(&line))
    }
}

impl PaneView<'_> {
    #[must_use]
    pub fn source_lines(&self) -> SourceLines {
        let buffer = self.buffer();
        let cursor = buffer.line_of(self.cursor());
        let lines = if matches!(self.mode, Mode::Visual(_)) {
            let anchor = buffer.line_of(self.selection.anchor);
            anchor.min(cursor)..=anchor.max(cursor)
        } else {
            cursor..=cursor
        };
        let block = self
            .document
            .layout()
            .map(Layout::styles)
            .and_then(|styles| styles.math_block(cursor));
        SourceLines { lines, block }
    }
}
